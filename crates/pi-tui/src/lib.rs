use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use pi_core::{AgentLoop, TurnEvent};
use pi_providers::{ModelConfig, PiConfig};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, ListState, Paragraph, Wrap},
    Terminal,
};
use std::fs;
use std::io;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;

pub mod account_picker;
pub mod anim;
pub mod autocomplete;
pub mod diff_view;
pub mod help_overlay;
pub mod markdown;
pub mod mermaid;
pub mod messages;
pub mod model_picker;
pub mod provider_picker;
pub mod session_picker;
pub mod style;
pub mod theme_picker;
pub mod tool_display;
pub mod usage_overlay;

pub use account_picker::AccountPicker;
pub use anim::Spinner;
pub use autocomplete::AutocompleteEngine;
pub use diff_view::{DiffLine, DiffView, DiffViewState};
pub use help_overlay::{HelpOverlay, HelpOverlayState};
pub use markdown::MarkdownRenderer;
pub use mermaid::MermaidRenderer;
pub use messages::{Message, MessageRenderer};
pub use model_picker::ModelPickerWidget;
pub use provider_picker::ProviderPickerWidget;
pub use session_picker::SessionPicker;
pub use style::{Theme, ThemeKind, ThemePalette};
pub use theme_picker::ThemePickerWidget;
pub use usage_overlay::UsageOverlay;

#[derive(Debug)]
pub enum AgentTaskEvent {
    Event(TurnEvent),
    Finished(Result<String, String>),
    ModelsRefreshed(Vec<pi_providers::ModelInfo>),
}

pub struct PiTuiApp {
    pub model_id: String,
    pub context_pct: f32,
    pub estimated_tokens: usize,
    pub input_text: String,
    pub cursor_pos: usize,
    pub prompt_history: Vec<String>,
    pub history_index: Option<usize>,
    pub scroll_offset: u16,
    pub history: Vec<(String, String)>, // (role, message)
    pub is_running: bool,
    pub has_agents_md: bool,
    pub agent_loop: Arc<Mutex<AgentLoop>>,
    pub queued_messages: Vec<String>,

    // Theme Engine
    pub theme: ThemePalette,

    // Async execution & streaming state
    pub active_turn: Option<JoinHandle<()>>,
    pub is_agent_running: bool,
    pub event_tx: mpsc::UnboundedSender<AgentTaskEvent>,
    pub event_rx: mpsc::UnboundedReceiver<AgentTaskEvent>,
    pub spinner: Spinner,

    // Interactive Overlays
    pub show_model_picker: bool,
    pub all_catalog_models: Vec<pi_providers::ModelInfo>,
    pub model_search_query: String,
    pub model_picker_state: ListState,

    pub show_provider_picker: bool,
    pub provider_search_query: String,
    pub provider_picker_state: ListState,

    pub show_theme_picker: bool,
    pub theme_search_query: String,
    pub theme_picker_state: ListState,

    pub show_help_overlay: bool,
    pub help_overlay_state: HelpOverlayState,

    pub autocomplete_selected: usize,

    pub show_tree_overlay: bool,
    pub tree_overlay_state: ListState,

    pub show_auth_modal: bool,
    pub auth_provider: String,
    pub auth_input: String,
    pub auth_cursor: usize,

    pub show_diff_overlay: bool,
    pub diff_view_state: Option<DiffViewState>,

    pub expand_tools: bool,
    pub show_thinking: bool,
    pub turn_start_time: Option<std::time::Instant>,
}

impl PiTuiApp {
    pub fn new(model_id: &str) -> Self {
        let has_agents_md = Path::new("AGENTS.md").exists();
        let model_cfg = ModelConfig::resolve(model_id);
        let auth_provider = model_cfg.provider.clone();
        let needs_auth = model_cfg.provider != "ollama"
            && model_cfg.provider != "llamacpp"
            && model_cfg.provider != "lmstudio"
            && model_cfg.provider != "opencode"
            && model_cfg.provider != "openrouter"
            && model_cfg.api_key.is_empty();
        let agent_loop = AgentLoop::new(model_cfg);

        let all_catalog_models = pi_providers::ModelCatalogLoader::load_cached_or_static();

        let mut model_picker_state = ListState::default();
        model_picker_state.select(Some(0));

        let mut provider_picker_state = ListState::default();
        provider_picker_state.select(Some(0));

        let mut theme_picker_state = ListState::default();
        theme_picker_state.select(Some(0));

        let (event_tx, event_rx) = mpsc::unbounded_channel();

        // Spawn background task to auto-fetch live model catalog across providers & local daemons
        let bg_tx = event_tx.clone();
        tokio::spawn(async move {
            let refreshed = pi_providers::ModelCatalogLoader::fetch_all_models(false).await;
            let _ = bg_tx.send(AgentTaskEvent::ModelsRefreshed(refreshed));
        });

        Self {
            model_id: model_id.to_string(),
            context_pct: 0.0,
            estimated_tokens: 0,
            input_text: String::new(),
            cursor_pos: 0,
            prompt_history: Vec::new(),
            history_index: None,
            scroll_offset: 0,
            history: {
                let cwd = std::env::current_dir().unwrap_or_default();
                let cwd_str = cwd.to_string_lossy();
                let display_cwd = if let Ok(home) = std::env::var("HOME") {
                    if cwd_str.starts_with(&home) {
                        cwd_str.replacen(&home, "~", 1)
                    } else {
                        cwd_str.into_owned()
                    }
                } else {
                    cwd_str.into_owned()
                };
                let mut h = vec![
                    (
                        "system".to_string(),
                        "  ████████╗ █████╗ ██╗   ██╗\n  ╚══██╔══╝██╔══██╗██║   ██║\n     ██║   ███████║██║   ██║\n     ██║   ██╔══██║██║   ██║\n     ██║   ██║  ██║╚██████╔╝\n     ╚═╝   ╚═╝  ╚═╝ ╚═════╝ ".to_string(),
                    ),
                    (
                        "path".to_string(),
                        display_cwd,
                    ),
                ];
                if has_agents_md {
                    h.push((
                        "system".to_string(),
                        "[Context]\n  AGENTS.md".to_string(),
                    ));
                }
                let skill_registry = pi_core::skills::SkillRegistry::new();
                if !skill_registry.skills.is_empty() {
                    let skills_names = skill_registry.skills.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(", ");
                    h.push((
                        "system".to_string(),
                        format!("[Skills]\n  {}", skills_names),
                    ));
                }
                h.push((
                    "system".to_string(),
                    "[Extensions]\n  read, write, edit, bash, grep, find, ls, web_fetch, web_search, git, github, lsp, ast, mcp".to_string(),
                ));
                h
            },
            is_running: true,
            has_agents_md,
            agent_loop: Arc::new(Mutex::new(agent_loop)),
            queued_messages: Vec::new(),
            theme: ThemePalette::default_pi(),
            active_turn: None,
            is_agent_running: false,
            event_tx,
            event_rx,
            spinner: Spinner::new(),
            show_model_picker: false,
            all_catalog_models,
            model_search_query: String::new(),
            model_picker_state,
            show_provider_picker: false,
            provider_search_query: String::new(),
            provider_picker_state,
            show_theme_picker: false,
            theme_search_query: String::new(),
            theme_picker_state,
            show_help_overlay: false,
            help_overlay_state: HelpOverlayState::new(),
            autocomplete_selected: 0,
            show_tree_overlay: false,
            tree_overlay_state: ListState::default(),
            show_auth_modal: needs_auth,
            auth_provider,
            auth_input: String::new(),
            auth_cursor: 0,
            show_diff_overlay: false,
            diff_view_state: None,
            expand_tools: true,
            show_thinking: true,
            turn_start_time: None,
        }
    }

    fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(r);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }

    pub fn abort_active_turn(&mut self) {
        if let Some(handle) = self.active_turn.take() {
            handle.abort();
        }
        self.is_agent_running = false;
        self.turn_start_time = None;
        while self.event_rx.try_recv().is_ok() {}
        self.history.push(("system".to_string(), "Interrupted active execution.".to_string()));
    }

    pub fn start_agent_turn(&mut self, text: String) {
        if self.is_agent_running {
            self.abort_active_turn();
        }

        self.history.push(("user".to_string(), text.clone()));
        self.scroll_offset = 0;
        self.is_agent_running = true;
        self.turn_start_time = Some(std::time::Instant::now());

        let agent_loop_arc = Arc::clone(&self.agent_loop);
        let event_tx = self.event_tx.clone();

        let handle = tokio::spawn(async move {
            let mut guard = agent_loop_arc.lock().await;
            let tx_clone = event_tx.clone();
            let result = guard
                .run_turn(&text, move |evt| {
                    let _ = tx_clone.send(AgentTaskEvent::Event(evt));
                })
                .await;

            let _ = event_tx.send(AgentTaskEvent::Finished(
                result.map_err(|e| e.to_string()),
            ));
        });

        self.active_turn = Some(handle);
    }

    pub fn poll_agent_events(&mut self) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                AgentTaskEvent::Event(turn_event) => match turn_event {
                    TurnEvent::ContextPrepared { token_estimate } => {
                        self.estimated_tokens = token_estimate;
                        self.context_pct = (token_estimate as f32 / 128_000.0) * 100.0;
                    }
                    TurnEvent::ModelStreaming { chunk } => {
                        if let Some((role, msg)) = self.history.last_mut() {
                            if role == "pi" {
                                msg.push_str(&chunk);
                            } else {
                                self.history.push(("pi".to_string(), chunk));
                            }
                        } else {
                            self.history.push(("pi".to_string(), chunk));
                        }
                    }
                    TurnEvent::ToolExecuting { tool_name, tool_call_id } => {
                        self.history.push((
                            "tool".to_string(),
                            format!("Executing tool [{}] (call_id: {})", tool_name, tool_call_id),
                        ));
                    }
                    TurnEvent::ToolCompleted { tool_name, is_error } => {
                        self.history.push((
                            "tool".to_string(),
                            format!("Tool [{}] completed (error: {})", tool_name, is_error),
                        ));
                    }
                    TurnEvent::ContextCompacted { old_turns, new_summary_len } => {
                        self.history.push((
                            "system".to_string(),
                            format!("Context compacted (old turns: {}, summary: {} bytes)", old_turns, new_summary_len),
                        ));
                    }
                    TurnEvent::TurnCompleted { total_tokens } => {
                        self.estimated_tokens = total_tokens;
                        self.context_pct = (total_tokens as f32 / 128_000.0) * 100.0;
                    }
                },
                AgentTaskEvent::Finished(result) => {
                    self.is_agent_running = false;
                    self.active_turn = None;

                    match result {
                        Ok(response) => {
                            let mut handled = false;
                            for (role, msg) in self.history.iter_mut().rev() {
                                if role == "pi" {
                                    if msg.is_empty() || (response.contains("Tool Output:\n") && !msg.contains("Tool Output:\n")) {
                                        *msg = response.clone();
                                    }
                                    handled = true;
                                    break;
                                } else if role == "user" {
                                    break;
                                }
                            }
                            if !handled && !response.is_empty() {
                                self.history.push(("pi".to_string(), response));
                            }
                        }
                        Err(err) => {
                            self.history.push(("system".to_string(), format!("Agent Error: {}", err)));
                        }
                    }

                    // Process queued messages if any
                    if !self.queued_messages.is_empty() {
                        let next_queued = self.queued_messages.remove(0);
                        self.history.push(("system".to_string(), format!("[Queued Execution] {}", next_queued)));
                        self.start_agent_turn(next_queued);
                    }
                }
                AgentTaskEvent::ModelsRefreshed(models) => {
                    self.all_catalog_models = models;
                }
            }
        }
    }

    pub async fn handle_slash_command(&mut self, text: &str) {
        let parts: Vec<&str> = text.split_whitespace().collect();
        let cmd = parts.first().copied().unwrap_or("");

        match cmd {
            "/help" | "/hotkeys" => {
                self.help_overlay_state = HelpOverlayState::new();
                self.show_help_overlay = true;
            }
            "/theme" => {
                if parts.len() > 1 {
                    let theme_arg = parts[1];
                    if let Some(kind) = ThemeKind::parse(theme_arg) {
                        self.theme = ThemePalette::from_kind(kind);
                        self.history.push((
                            "system".to_string(),
                            format!("Switched theme to {} ({})", self.theme.kind.name(), self.theme.kind.id_str()),
                        ));
                    } else {
                        self.history.push((
                            "system".to_string(),
                            format!("Unknown theme: '{}'. Available themes: default, tokyonight, catppuccin, gruvbox, nord, solarized, onedark", theme_arg),
                        ));
                    }
                } else {
                    self.theme_search_query.clear();
                    self.theme_picker_state.select(Some(0));
                    self.show_theme_picker = true;
                }
            }
            "/skills" => {
                let skill_registry = pi_core::skills::SkillRegistry::new();
                if skill_registry.skills.is_empty() {
                    self.history.push(("system".to_string(), "No agent skills discovered.".to_string()));
                } else {
                    let mut summary = format!("Discovered Skills ({} total):\n", skill_registry.skills.len());
                    for s in &skill_registry.skills {
                        summary.push_str(&format!("  • {}: {}\n", s.name, s.description));
                    }
                    self.history.push(("system".to_string(), summary));
                }
            }
            "/login" | "/auth" => {
                if parts.len() > 2 {
                    let provider = parts[1];
                    let key = parts[2].to_string();
                    let _ = pi_providers::AuthResolver::save_key(provider, &key);
                    if let Ok(mut guard) = self.agent_loop.try_lock()
                        && guard.model_config.provider == provider
                    {
                        guard.model_config.api_key = key;
                    }
                    self.history.push(("system".to_string(), format!("Credentials saved for provider [{}]", provider)));
                } else if parts.len() > 1 {
                    let provider = parts[1];
                    self.auth_provider = provider.to_string();
                    let instructions = pi_providers::AuthResolver::get_login_instructions(provider);
                    self.history.push(("system".to_string(), instructions));
                    self.show_auth_modal = true;
                } else {
                    // Open full interactive Provider Picker across all providers
                    self.provider_search_query.clear();
                    self.provider_picker_state.select(Some(0));
                    self.show_provider_picker = true;
                }
            }
            "/refresh" => {
                let tx = self.event_tx.clone();
                self.history.push(("system".to_string(), "Refreshing model catalogs in background...".to_string()));
                tokio::spawn(async move {
                    let models = pi_providers::ModelCatalogLoader::fetch_all_models(true).await;
                    let _ = tx.send(AgentTaskEvent::ModelsRefreshed(models));
                });
            }
            "/model" | "/models" => {
                if parts.len() > 1 {
                    let new_model = parts[1];
                    self.model_id = new_model.to_string();
                    let new_cfg = ModelConfig::resolve(new_model);
                    let provider = new_cfg.provider.clone();
                    if let Ok(mut guard) = self.agent_loop.try_lock() {
                        guard.model_config = new_cfg;
                    }
                    self.history.push((
                        "system".to_string(),
                        format!("Switched model to {} (Provider: {})", self.model_id, provider),
                    ));
                } else {
                    self.model_search_query.clear();
                    self.model_picker_state.select(Some(0));
                    self.show_model_picker = true;
                }
            }
            "/provider" | "/providers" => {
                if parts.len() > 1 {
                    self.provider_search_query = parts[1].to_string();
                } else {
                    self.provider_search_query.clear();
                }
                self.provider_picker_state.select(Some(0));
                self.show_provider_picker = true;
            }
            "/export" => {
                let path = parts.get(1).cloned().unwrap_or("session_export.md");
                let export_content: Vec<String> = self.history.iter().map(|(r, m)| format!("### {}\n{}", r, m)).collect();
                if fs::write(path, export_content.join("\n\n")).is_ok() {
                    self.history.push(("system".to_string(), format!("Session exported to {}", path)));
                } else {
                    self.history.push(("system".to_string(), format!("Failed to export to {}", path)));
                }
            }
            "/diff" => {
                if parts.len() > 1 {
                    let path = parts[1];
                    let file_path_obj = Path::new(path);
                    if file_path_obj.exists() {
                        let new_content = fs::read_to_string(path).unwrap_or_default();
                        let old_content = match std::process::Command::new("git")
                            .args(["show", &format!("HEAD:{}", path)])
                            .output()
                        {
                            Ok(out) if out.status.success() => {
                                String::from_utf8_lossy(&out.stdout).to_string()
                            }
                            _ => String::new(),
                        };
                        let state = DiffViewState::new(path, &old_content, &new_content, false);
                        self.diff_view_state = Some(state);
                        self.show_diff_overlay = true;
                    } else {
                        let old_content = match std::process::Command::new("git")
                            .args(["show", &format!("HEAD:{}", path)])
                            .output()
                        {
                            Ok(out) if out.status.success() => {
                                String::from_utf8_lossy(&out.stdout).to_string()
                            }
                            _ => String::new(),
                        };
                        if !old_content.is_empty() {
                            let state = DiffViewState::new(path, &old_content, "", false);
                            self.diff_view_state = Some(state);
                            self.show_diff_overlay = true;
                        } else {
                            self.history.push(("system".to_string(), format!("File not found: {}", path)));
                        }
                    }
                } else {
                    let name_only = match std::process::Command::new("git")
                        .args(["diff", "--name-only", "HEAD"])
                        .output()
                    {
                        Ok(out) if out.status.success() => {
                            String::from_utf8_lossy(&out.stdout).to_string()
                        }
                        _ => String::new(),
                    };
                    let modified_files: Vec<&str> = name_only.lines().filter(|l| !l.is_empty()).collect();
                    if let Some(first_file) = modified_files.first() {
                        let new_content = fs::read_to_string(first_file).unwrap_or_default();
                        let old_content = match std::process::Command::new("git")
                            .args(["show", &format!("HEAD:{}", first_file)])
                            .output()
                        {
                            Ok(out) if out.status.success() => {
                                String::from_utf8_lossy(&out.stdout).to_string()
                            }
                            _ => String::new(),
                        };
                        let mut state = DiffViewState::new(first_file, &old_content, &new_content, false);
                        if modified_files.len() > 1 {
                            state.title = format!(" Diff: {} (1 of {} files) ", first_file, modified_files.len());
                        }
                        self.diff_view_state = Some(state);
                        self.show_diff_overlay = true;
                        if modified_files.len() > 1 {
                            self.history.push((
                                "system".to_string(),
                                format!("Showing diff for {} (Modified files: {})", first_file, modified_files.join(", ")),
                            ));
                        }
                    } else {
                        self.history.push(("system".to_string(), "No modified files found in git workspace.".to_string()));
                    }
                }
            }
            "/replay" => {
                if let Ok(guard) = self.agent_loop.try_lock() {
                    let history = guard.session_tree.get_active_branch_history();
                    let mut replay_summary = format!("Session Turn Replay ({} nodes total):\n", history.len());
                    for (i, node) in history.iter().enumerate() {
                        let preview = node.content.lines().next().unwrap_or("");
                        let short_preview = if preview.len() > 60 {
                            &preview[..preview.floor_char_boundary(60)]
                        } else {
                            preview
                        };
                        replay_summary.push_str(&format!("  Step {:02}: [{:?}] {}\n", i + 1, node.role, short_preview));
                    }
                    self.history.push(("system".to_string(), replay_summary));
                } else {
                    self.history.push(("system".to_string(), "Cannot replay session while turn is active.".to_string()));
                }
            }
            "/share" => {
                if let Ok(guard) = self.agent_loop.try_lock() {
                    let history = guard.session_tree.get_active_branch_history();
                    let share_json = serde_json::json!({
                        "session_id": guard.session_tree.session_id,
                        "node_count": history.len(),
                        "nodes": history,
                    });
                    let _ = fs::write("session_share.json", serde_json::to_string_pretty(&share_json).unwrap_or_default());
                    self.history.push(("system".to_string(), "Share payload exported to session_share.json".to_string()));
                } else {
                    self.history.push(("system".to_string(), "Cannot export share payload while turn is in-flight".to_string()));
                }
            }
            "/session" => {
                if let Ok(guard) = self.agent_loop.try_lock() {
                    let history = guard.session_tree.get_active_branch_history();
                    let approx_tokens: usize = history.iter().map(|n| n.content.len() / 4).sum();
                    let pct = (approx_tokens as f32 / 128_000.0) * 100.0;
                    self.estimated_tokens = approx_tokens;
                    self.context_pct = pct;
                    self.history.push((
                        "system".to_string(),
                        format!("Session Info:\n  ID: {}\n  Nodes: {}\n  Estimated Tokens: {} / 128,000\n  Context Capacity: {:.2}%\n  Active Theme: {}", guard.session_tree.session_id, history.len(), approx_tokens, pct, self.theme.kind.name()),
                    ));
                } else {
                    self.history.push((
                        "system".to_string(),
                        format!("Session Info: Context Capacity approx {:.2}%", self.context_pct),
                    ));
                }
            }
            "/tree" => {
                self.show_tree_overlay = true;
            }
            "/fork" => {
                if let Ok(mut guard) = self.agent_loop.try_lock() {
                    let new_id = guard.session_tree.append_child(pi_session::Role::System, "Forked branch point".to_string());
                    self.history.push(("system".to_string(), format!("Created new fork branch at node [{}]", &new_id[..6.min(new_id.len())])));
                }
            }
            "/new" => {
                if let Ok(mut guard) = self.agent_loop.try_lock() {
                    guard.session_tree = pi_session::SessionTree::new();
                    self.history.clear();
                    self.estimated_tokens = 0;
                    self.context_pct = 0.0;
                    self.history.push(("system".to_string(), "Started new session.".to_string()));
                }
            }
            "/compact" => {
                self.history.push(("system".to_string(), "Triggered context window compaction...".to_string()));
            }
            "/tools" => {
                self.expand_tools = !self.expand_tools;
                self.history.push((
                    "system".to_string(),
                    format!("Tool execution drawer: {}", if self.expand_tools { "expanded" } else { "collapsed" }),
                ));
            }
            "/thinking" => {
                self.show_thinking = !self.show_thinking;
                self.history.push((
                    "system".to_string(),
                    format!("Model thinking blocks: {}", if self.show_thinking { "visible" } else { "hidden" }),
                ));
            }
            "/clear" => {
                self.history.clear();
                self.history.push(("system".to_string(), "Cleared terminal transcript.".to_string()));
            }
            "/mcp" => {
                let mcp_mgr = pi_core::get_mcp_manager();
                if parts.len() > 1 && (parts[1] == "refresh" || parts[1] == "scan") {
                    let mut mgr = mcp_mgr.lock().await;
                    mgr.discover_servers();
                    let count = mgr.servers.len();
                    self.history.push(("system".to_string(), format!("Refreshed MCP servers: {} servers discovered across agents.", count)));
                } else {
                    let mgr = mcp_mgr.lock().await;
                    let mut summary = format!("Auto-Discovered MCP Servers ({} total):\n", mgr.servers.len());
                    for (name, srv) in &mgr.servers {
                        let target = srv.command.as_deref().or(srv.url.as_deref()).unwrap_or("unknown");
                        summary.push_str(&format!("  • [{}] {} ({:?}): {}\n", srv.source_agent, name, srv.transport, target));
                    }
                    summary.push_str("\nTip: Use `/mcp refresh` to rescan configuration paths.");
                    self.history.push(("system".to_string(), summary));
                }
            }
            "/reload" => {
                if let Ok(mut guard) = self.agent_loop.try_lock() {
                    guard.system_engine = pi_core::SystemPromptEngine::new();
                }
                self.has_agents_md = Path::new("AGENTS.md").exists();
                self.history.push(("system".to_string(), format!("Reloaded settings. AGENTS.md: {}", self.has_agents_md)));
            }
            "/quit" => {
                self.is_running = false;
            }
            _ => {
                self.history.push(("system".to_string(), format!("Unknown command: {}. Type /help for list.", cmd)));
            }
        }
    }

    pub fn render_token_progress_bar(&self, width: usize) -> Line<'static> {
        let clamped = self.context_pct.clamp(0.0, 100.0);
        let filled_chars = ((clamped / 100.0) * (width as f32)).round() as usize;
        let empty_chars = width.saturating_sub(filled_chars);

        let color = if clamped >= 80.0 {
            self.theme.red
        } else if clamped >= 50.0 {
            self.theme.yellow
        } else {
            self.theme.green
        };

        Line::from(vec![
            Span::styled("[", Style::default().fg(self.theme.muted)),
            Span::styled("█".repeat(filled_chars), Style::default().fg(color)),
            Span::styled("░".repeat(empty_chars), Style::default().fg(self.theme.border)),
            Span::styled(format!("] {:.0}% / 128k", clamped), Style::default().fg(self.theme.text)),
        ])
    }

    pub async fn run_loop(&mut self) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        while self.is_running {
            // Process all pending background agent events
            self.poll_agent_events();

            let spinner_frame = if self.is_agent_running {
                self.spinner.tick()
            } else {
                ""
            };

            terminal.draw(|f| {
                // Fill entire terminal with active theme background
                let bg_block = Block::default().style(Style::default().bg(self.theme.bg));
                f.render_widget(bg_block, f.area());

                let input_line_count = (self.input_text.lines().count().max(1) as u16).min(6);

                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Min(4),                         // Chat transcript
                        Constraint::Length(1),                      // ─── separator line
                        Constraint::Length(input_line_count),       // Prompt input
                        Constraint::Length(1),                      // Status bar line 1
                        Constraint::Length(1),                      // Status bar line 2
                    ])
                    .split(f.area());

                // Build Structured Transcript with Markdown and Mermaid rendering
                let mut lines: Vec<Line> = Vec::new();
                for (role, msg) in &self.history {
                    lines.extend(MessageRenderer::render_message(
                        role,
                        msg,
                        self.expand_tools,
                        self.show_thinking,
                    ));
                }

                // Streaming indicator appended to end of transcript
                if self.is_agent_running {
                    let elapsed_sec = self.turn_start_time.map(|t| t.elapsed().as_secs_f32()).unwrap_or(0.0);
                    lines.push(Line::from(vec![
                        Span::styled(format!("{} ", spinner_frame), Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD)),
                        Span::styled(format!("Streaming... {:.1}s", elapsed_sec), Style::default().fg(self.theme.muted)),
                    ]));
                }

                let total_lines = lines.len();
                let available_height = chunks[0].height as usize;
                let max_scroll_from_top = total_lines.saturating_sub(available_height);
                let effective_scroll = (max_scroll_from_top.saturating_sub(self.scroll_offset as usize)) as u16;

                let transcript = Paragraph::new(lines)
                    .wrap(Wrap { trim: true })
                    .scroll((effective_scroll, 0))
                    .style(Style::default().bg(self.theme.bg).fg(self.theme.text));
                f.render_widget(transcript, chunks[0]);

                // ─── Horizontal separator line above prompt
                let separator_width = chunks[1].width as usize;
                let sep_line = "─".repeat(separator_width);
                let separator = Paragraph::new(sep_line)
                    .style(Style::default().fg(self.theme.border).bg(self.theme.bg));
                f.render_widget(separator, chunks[1]);

                // Prompt Input — 'tau > ' prefix with block cursor
                let (display_input, input_style) = if self.input_text.is_empty() {
                    (
                        String::from("tau > █"),
                        Style::default().fg(self.theme.text).bg(self.theme.bg),
                    )
                } else {
                    let chars: Vec<char> = self.input_text.chars().collect();
                    let mut s = String::from("tau > ");
                    for (i, ch) in chars.iter().enumerate() {
                        if i == self.cursor_pos {
                            s.push('█');
                        }
                        s.push(*ch);
                    }
                    if self.cursor_pos >= chars.len() {
                        s.push('█');
                    }
                    (s, Style::default().fg(self.theme.text).bg(self.theme.bg))
                };

                let input = Paragraph::new(display_input)
                    .wrap(Wrap { trim: false })
                    .style(input_style);
                f.render_widget(input, chunks[2]);

                // === Enhanced Two-line Live Status & Shortcut Bar ===
                let cwd = std::env::current_dir().unwrap_or_default();
                let cwd_str = cwd.to_string_lossy();
                let display_cwd = if let Ok(home) = std::env::var("HOME") {
                    if cwd_str.starts_with(&home) {
                        cwd_str.replacen(&home, "~", 1)
                    } else {
                        cwd_str.into_owned()
                    }
                } else {
                    cwd_str.into_owned()
                };

                let git_branch = match std::process::Command::new("git")
                    .args(["branch", "--show-current"])
                    .output()
                {
                    Ok(out) if out.status.success() => {
                        String::from_utf8_lossy(&out.stdout).trim().to_string()
                    }
                    _ => String::new(),
                };

                let git_status_count = match std::process::Command::new("git")
                    .args(["status", "--porcelain"])
                    .output()
                {
                    Ok(out) if out.status.success() => {
                        let text = String::from_utf8_lossy(&out.stdout);
                        let count = text.lines().filter(|l| !l.is_empty()).count();
                        if count > 0 {
                            format!("{} modified", count)
                        } else {
                            "clean".to_string()
                        }
                    }
                    _ => String::new(),
                };

                let provider_name = if let Ok(guard) = self.agent_loop.try_lock() {
                    guard.model_config.provider.clone()
                } else {
                    self.model_id.split('/').next().unwrap_or("unknown").to_string()
                };

                let status_width = chunks[3].width as usize;

                // Status Line 1:
                // Left: " 📁 ~/<cwd> · ⚡ Tau (τ)"
                // Right: "[<model_id>] · [<provider>] · [🎨 <theme>] "
                let left1 = format!(" 📁 {} · ⚡ Tau (τ)", display_cwd);
                let right1 = format!("🤖 {} · 🌐 {} · 🎨 {} ", self.model_id, provider_name, self.theme.kind.name());
                let pad1 = status_width.saturating_sub(left1.len() + right1.len());
                let status_line_1 = Line::from(vec![
                    Span::styled(left1, self.theme.status_bar_accent()),
                    Span::styled(" ".repeat(pad1), self.theme.status_bar()),
                    Span::styled(right1, self.theme.status_bar_accent()),
                ]);
                let status1 = Paragraph::new(status_line_1).style(self.theme.status_bar());
                f.render_widget(status1, chunks[3]);

                // Status Line 2:
                // Left: Token bar: [████████░░░░░░░░] 42% / 128k · $0.00
                // Right: [Ctrl+L: Models] [Ctrl+T: Themes] [?: Help] [Esc: Cancel] · <branch> (<status>)
                let clamped_pct = self.context_pct.clamp(0.0, 100.0);
                let bar_color = if clamped_pct >= 80.0 {
                    self.theme.red
                } else if clamped_pct >= 50.0 {
                    self.theme.yellow
                } else {
                    self.theme.green
                };

                let filled = ((clamped_pct / 100.0) * 12.0).round() as usize;
                let empty = 12usize.saturating_sub(filled);
                let git_info = if !git_branch.is_empty() {
                    format!(" · 🌿 {} ({})", git_branch, git_status_count)
                } else {
                    String::new()
                };
                let shortcuts_str = format!("[Ctrl+L: Models] [Ctrl+T: Themes] [?: Help] [Esc: Cancel]{} ", git_info);

                let left_spans = vec![
                    Span::styled(" ", self.theme.status_bar()),
                    Span::styled("[", Style::default().fg(self.theme.muted).bg(self.theme.bg)),
                    Span::styled("█".repeat(filled), Style::default().fg(bar_color).bg(self.theme.bg)),
                    Span::styled("░".repeat(empty), Style::default().fg(self.theme.border).bg(self.theme.bg)),
                    Span::styled(format!("] {:.0}% / 128k · $0.00", clamped_pct), self.theme.status_bar()),
                ];

                let left_len = 1 + 1 + 12 + 1 + format!(" {:.0}% / 128k · $0.00", clamped_pct).len();
                let pad2 = status_width.saturating_sub(left_len + shortcuts_str.len());

                let mut status2_spans = left_spans;
                status2_spans.push(Span::styled(" ".repeat(pad2), self.theme.status_bar()));
                status2_spans.push(Span::styled(shortcuts_str, self.theme.status_bar_accent()));

                let status2 = Paragraph::new(Line::from(status2_spans)).style(self.theme.status_bar());
                f.render_widget(status2, chunks[4]);

                // Render Floating Slash Command Autocomplete Popup
                if !self.show_model_picker
                    && !self.show_provider_picker
                    && !self.show_theme_picker
                    && !self.show_help_overlay
                    && !self.show_auth_modal
                    && !self.show_tree_overlay
                    && !self.show_diff_overlay
                    && self.input_text.starts_with('/')
                {
                    let suggestions = AutocompleteEngine::get_suggestions(&self.input_text);
                    AutocompleteEngine::render_popup(f, &suggestions, self.autocomplete_selected, chunks[2]);
                }

                // Render Interactive Auth Wizard Modal
                if self.show_auth_modal {
                    let area = Self::centered_rect(65, 30, f.area());
                    f.render_widget(Clear, area);
                    let modal = AccountPicker::render_modal(&self.auth_provider, &self.auth_input);
                    f.render_widget(modal, area);
                }

                // Render Interactive Searchable Model Picker Dialog Overlay
                if self.show_model_picker {
                    let area = Self::centered_rect(75, 55, f.area());
                    f.render_widget(Clear, area);

                    let filtered: Vec<pi_providers::ModelInfo> = pi_providers::ModelCatalogLoader::search_models(&self.all_catalog_models, &self.model_search_query)
                        .into_iter()
                        .cloned()
                        .collect();

                    let (search_bar, list, block) = ModelPickerWidget::render(
                        &self.model_search_query,
                        &filtered,
                        self.all_catalog_models.len(),
                    );

                    let inner_chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Length(2), Constraint::Min(4)])
                        .margin(1)
                        .split(area);

                    f.render_widget(block, area);
                    f.render_widget(search_bar, inner_chunks[0]);
                    f.render_stateful_widget(list, inner_chunks[1], &mut self.model_picker_state);
                }

                // Render Interactive Searchable Provider Picker Dialog Overlay
                if self.show_provider_picker {
                    let area = Self::centered_rect(75, 55, f.area());
                    f.render_widget(Clear, area);

                    let q = self.provider_search_query.to_lowercase();
                    let filtered: Vec<&'static provider_picker::ProviderEntry> = provider_picker::ALL_PROVIDERS
                        .iter()
                        .filter(|p| p.id.to_lowercase().contains(&q) || p.name.to_lowercase().contains(&q))
                        .collect();

                    let (search_bar, list, block) = ProviderPickerWidget::render(
                        &self.provider_search_query,
                        &filtered,
                    );

                    let inner_chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Length(2), Constraint::Min(4)])
                        .margin(1)
                        .split(area);

                    f.render_widget(block, area);
                    f.render_widget(search_bar, inner_chunks[0]);
                    f.render_stateful_widget(list, inner_chunks[1], &mut self.provider_picker_state);
                }

                // Render Interactive Searchable Theme Picker Dialog Overlay
                if self.show_theme_picker {
                    let area = Self::centered_rect(75, 55, f.area());
                    f.render_widget(Clear, area);

                    let filtered = ThemePickerWidget::filter_themes(&self.theme_search_query);

                    let (search_bar, list, block) = ThemePickerWidget::render(
                        &self.theme_search_query,
                        &filtered,
                        self.theme.kind,
                        &self.theme,
                    );

                    let inner_chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Length(2), Constraint::Min(4)])
                        .margin(1)
                        .split(area);

                    f.render_widget(block, area);
                    f.render_widget(search_bar, inner_chunks[0]);
                    f.render_stateful_widget(list, inner_chunks[1], &mut self.theme_picker_state);
                }

                // Render Interactive Help Cheatsheet Overlay
                if self.show_help_overlay {
                    let area = Self::centered_rect(80, 75, f.area());
                    f.render_widget(Clear, area);
                    HelpOverlay::render(f, area, &self.help_overlay_state, &self.theme);
                }

                // Render Interactive Session Tree Navigator Overlay
                if self.show_tree_overlay {
                    let area = Self::centered_rect(70, 50, f.area());
                    f.render_widget(Clear, area);

                    let raw_items: Vec<String> = if let Ok(guard) = self.agent_loop.try_lock() {
                        let history = guard.session_tree.get_active_branch_history();
                        history
                            .iter()
                            .enumerate()
                            .map(|(idx, node)| {
                                let prefix = if idx == history.len().saturating_sub(1) { "└" } else { "├" };
                                let preview = node.content.lines().next().unwrap_or("");
                                format!("{} [{}] {:?}: {}", prefix, &node.id[..6.min(node.id.len())], node.role, preview)
                            })
                            .collect()
                    } else {
                        Vec::new()
                    };

                    let (list, block) = SessionPicker::render_widget(&raw_items);
                    let list_widget = list.block(block);
                    f.render_stateful_widget(list_widget, area, &mut self.tree_overlay_state);
                }

                // Render Interactive Terminal Diff Visualizer Overlay
                if self.show_diff_overlay && let Some(ref state) = self.diff_view_state {
                    let area = Self::centered_rect(85, 75, f.area());
                    f.render_widget(Clear, area);
                    DiffView::render(state, f, area);
                }
            })?;

            // Responsive polling: 25ms allows real-time token rendering and instant interruption
            if event::poll(std::time::Duration::from_millis(25))? {
                let Event::Key(key) = event::read()? else {
                    continue;
                };

                // Handle Help Overlay Keys
                if self.show_help_overlay {
                    match key.code {
                        KeyCode::Esc | KeyCode::F(1) => {
                            self.show_help_overlay = false;
                        }
                        KeyCode::Char('q') if self.help_overlay_state.search_query.is_empty() => {
                            self.show_help_overlay = false;
                        }
                        KeyCode::Up => {
                            self.help_overlay_state.scroll_up(1);
                        }
                        KeyCode::Down => {
                            self.help_overlay_state.scroll_down(1, 45, 20);
                        }
                        KeyCode::PageUp => {
                            self.help_overlay_state.scroll_up(10);
                        }
                        KeyCode::PageDown => {
                            self.help_overlay_state.scroll_down(10, 45, 20);
                        }
                        KeyCode::Home => {
                            self.help_overlay_state.scroll_home();
                        }
                        KeyCode::End => {
                            self.help_overlay_state.scroll_end(45, 20);
                        }
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            self.help_overlay_state.search_query.clear();
                            self.help_overlay_state.scroll_home();
                        }
                        KeyCode::Char(c) => {
                            self.help_overlay_state.search_query.push(c);
                            self.help_overlay_state.scroll_home();
                        }
                        KeyCode::Backspace => {
                            self.help_overlay_state.search_query.pop();
                            self.help_overlay_state.scroll_home();
                        }
                        _ => {}
                    }
                    continue;
                }

                // Handle Theme Picker Overlay Keys
                if self.show_theme_picker {
                    let filtered = ThemePickerWidget::filter_themes(&self.theme_search_query);

                    match key.code {
                        KeyCode::Esc => {
                            self.show_theme_picker = false;
                            self.theme_search_query.clear();
                        }
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            self.theme_search_query.clear();
                            self.theme_picker_state.select(Some(0));
                        }
                        KeyCode::Char(c) => {
                            self.theme_search_query.push(c);
                            self.theme_picker_state.select(Some(0));
                        }
                        KeyCode::Backspace => {
                            self.theme_search_query.pop();
                            self.theme_picker_state.select(Some(0));
                        }
                        KeyCode::Up => {
                            ThemePickerWidget::handle_navigation(&mut self.theme_picker_state, filtered.len(), true);
                        }
                        KeyCode::Down => {
                            ThemePickerWidget::handle_navigation(&mut self.theme_picker_state, filtered.len(), false);
                        }
                        KeyCode::Enter => {
                            if let Some(i) = self.theme_picker_state.selected()
                                && let Some(kind) = filtered.get(i)
                            {
                                self.theme = ThemePalette::from_kind(*kind);
                                self.history.push((
                                    "system".to_string(),
                                    format!("Applied theme: {} ({})", self.theme.kind.name(), self.theme.kind.id_str()),
                                ));
                            }
                            self.show_theme_picker = false;
                            self.theme_search_query.clear();
                        }
                        _ => {}
                    }
                    continue;
                }

                // Handle Diff Visualizer Overlay
                if self.show_diff_overlay {
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => {
                            self.show_diff_overlay = false;
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            if let Some(ref mut state) = self.diff_view_state {
                                state.scroll_up(1);
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            if let Some(ref mut state) = self.diff_view_state {
                                state.scroll_down(1, 25);
                            }
                        }
                        KeyCode::PageUp => {
                            if let Some(ref mut state) = self.diff_view_state {
                                state.scroll_up(10);
                            }
                        }
                        KeyCode::PageDown => {
                            if let Some(ref mut state) = self.diff_view_state {
                                state.scroll_down(10, 25);
                            }
                        }
                        KeyCode::Home => {
                            if let Some(ref mut state) = self.diff_view_state {
                                state.scroll_home();
                            }
                        }
                        KeyCode::End => {
                            if let Some(ref mut state) = self.diff_view_state {
                                state.scroll_end(25);
                            }
                        }
                        KeyCode::Char('y') | KeyCode::Enter => {
                            if let Some(ref state) = self.diff_view_state
                                && state.is_pending_review
                            {
                                let _ = fs::write(&state.file_path, &state.new_content);
                                self.history.push((
                                    "system".to_string(),
                                    format!("Accepted changes for {}", state.file_path),
                                ));
                            }
                            self.show_diff_overlay = false;
                        }
                        KeyCode::Char('n') => {
                            if let Some(ref state) = self.diff_view_state
                                && state.is_pending_review
                            {
                                self.history.push((
                                    "system".to_string(),
                                    format!("Rejected changes for {}", state.file_path),
                                ));
                            }
                            self.show_diff_overlay = false;
                        }
                        _ => {}
                    }
                    continue;
                }

                // Handle Auth Modal First
                if self.show_auth_modal {
                    match key.code {
                        KeyCode::Esc => {
                            self.show_auth_modal = false;
                            self.auth_input.clear();
                        }
                        KeyCode::Enter => {
                            let key_val = self.auth_input.trim().to_string();
                            if !key_val.is_empty() {
                                let mut cfg = PiConfig::load();
                                let provider = &self.auth_provider;
                                let _ = cfg.set_api_key(provider, key_val.clone());
                                if let Ok(mut guard) = self.agent_loop.try_lock() {
                                    guard.model_config.api_key = key_val;
                                }
                                self.history.push(("system".to_string(), format!("API key saved to ~/.pi/config.json for provider [{}]", provider)));
                            }
                            self.show_auth_modal = false;
                            self.auth_input.clear();
                        }
                        KeyCode::Char(c) => {
                            self.auth_input.push(c);
                        }
                        KeyCode::Backspace => {
                            self.auth_input.pop();
                        }
                        _ => {}
                    }
                    continue;
                }

                // Handle Model Picker Overlay
                if self.show_model_picker {
                    let filtered: Vec<pi_providers::ModelInfo> = pi_providers::ModelCatalogLoader::search_models(&self.all_catalog_models, &self.model_search_query)
                        .into_iter()
                        .cloned()
                        .collect();

                    match key.code {
                        KeyCode::Esc => {
                            self.show_model_picker = false;
                            self.model_search_query.clear();
                        }
                        KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            let tx = self.event_tx.clone();
                            self.history.push(("system".to_string(), "Refreshing model catalogs in background...".to_string()));
                            tokio::spawn(async move {
                                let models = pi_providers::ModelCatalogLoader::fetch_all_models(true).await;
                                let _ = tx.send(AgentTaskEvent::ModelsRefreshed(models));
                            });
                        }
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            self.model_search_query.clear();
                            self.model_picker_state.select(Some(0));
                        }
                        KeyCode::Char(c) => {
                            self.model_search_query.push(c);
                            self.model_picker_state.select(Some(0));
                        }
                        KeyCode::Backspace => {
                            self.model_search_query.pop();
                            self.model_picker_state.select(Some(0));
                        }
                        KeyCode::Up => {
                            ModelPickerWidget::handle_navigation(&mut self.model_picker_state, filtered.len(), true);
                        }
                        KeyCode::Down => {
                            ModelPickerWidget::handle_navigation(&mut self.model_picker_state, filtered.len(), false);
                        }
                        KeyCode::Enter => {
                            if let Some(i) = self.model_picker_state.selected()
                                && let Some(selected_model) = filtered.get(i)
                            {
                                self.model_id = selected_model.id.clone();
                                let new_cfg = ModelConfig::resolve(&selected_model.id);
                                let provider = new_cfg.provider.clone();
                                let api_key_empty = new_cfg.api_key.is_empty();
                                if let Ok(mut guard) = self.agent_loop.try_lock() {
                                    guard.model_config = new_cfg;
                                }

                                if provider != "ollama" && provider != "llamacpp" && provider != "lmstudio" && api_key_empty {
                                    self.auth_provider = provider.clone();
                                    self.show_auth_modal = true;
                                }

                                self.history.push((
                                    "system".to_string(),
                                    format!("Selected model: {} (Provider: {})", self.model_id, provider),
                                ));
                            }
                            self.show_model_picker = false;
                            self.model_search_query.clear();
                        }
                        _ => {}
                    }
                    continue;
                }

                // Handle Provider Picker Overlay
                if self.show_provider_picker {
                    let q = self.provider_search_query.to_lowercase();
                    let filtered: Vec<&'static provider_picker::ProviderEntry> = provider_picker::ALL_PROVIDERS
                        .iter()
                        .filter(|p| p.id.to_lowercase().contains(&q) || p.name.to_lowercase().contains(&q))
                        .collect();

                    match key.code {
                        KeyCode::Esc => {
                            self.show_provider_picker = false;
                            self.provider_search_query.clear();
                        }
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            self.provider_search_query.clear();
                            self.provider_picker_state.select(Some(0));
                        }
                        KeyCode::Char(c) => {
                            self.provider_search_query.push(c);
                            self.provider_picker_state.select(Some(0));
                        }
                        KeyCode::Backspace => {
                            self.provider_search_query.pop();
                            self.provider_picker_state.select(Some(0));
                        }
                        KeyCode::Up => {
                            ProviderPickerWidget::handle_navigation(&mut self.provider_picker_state, filtered.len(), true);
                        }
                        KeyCode::Down => {
                            ProviderPickerWidget::handle_navigation(&mut self.provider_picker_state, filtered.len(), false);
                        }
                        KeyCode::Enter => {
                            if let Some(i) = self.provider_picker_state.selected()
                                && let Some(selected_prov) = filtered.get(i)
                            {
                                let prov_id = selected_prov.id;
                                let has_auth = pi_providers::AuthResolver::resolve_key(prov_id).is_some();
                                if !selected_prov.is_local && !has_auth {
                                    self.auth_provider = prov_id.to_string();
                                    self.show_auth_modal = true;
                                } else {
                                    // Filter models to this provider
                                    self.model_search_query = format!("{}/", prov_id);
                                    self.model_picker_state.select(Some(0));
                                    self.show_model_picker = true;
                                }
                            }
                            self.show_provider_picker = false;
                            self.provider_search_query.clear();
                        }
                        _ => {}
                    }
                    continue;
                }

                if self.show_tree_overlay {
                    match key.code {
                        KeyCode::Esc => {
                            self.show_tree_overlay = false;
                        }
                        KeyCode::Up => {
                            let history_len = if let Ok(guard) = self.agent_loop.try_lock() {
                                guard.session_tree.get_active_branch_history().len()
                            } else {
                                0
                            };
                            SessionPicker::handle_navigation(&mut self.tree_overlay_state, history_len, true);
                        }
                        KeyCode::Down => {
                            let history_len = if let Ok(guard) = self.agent_loop.try_lock() {
                                guard.session_tree.get_active_branch_history().len()
                            } else {
                                0
                            };
                            SessionPicker::handle_navigation(&mut self.tree_overlay_state, history_len, false);
                        }
                        KeyCode::Enter => {
                            if let Some(i) = self.tree_overlay_state.selected()
                                && let Ok(mut guard) = self.agent_loop.try_lock()
                            {
                                let history = guard.session_tree.get_active_branch_history();
                                if let Some(target_node) = history.get(i) {
                                    let target_id = target_node.id.clone();
                                    guard.session_tree.rewind_to(&target_id);
                                    self.history.push(("system".to_string(), format!("Rewound active tree node to [{}]", &target_id[..6.min(target_id.len())])));
                                }
                            }
                            self.show_tree_overlay = false;
                        }
                        _ => {}
                    }
                    continue;
                }

                // Main Hotkey & Keybinding Handlers
                match key.code {
                    KeyCode::F(1) => {
                        self.help_overlay_state = HelpOverlayState::new();
                        self.show_help_overlay = true;
                    }
                    KeyCode::Char('?') if self.input_text.is_empty() => {
                        self.help_overlay_state = HelpOverlayState::new();
                        self.show_help_overlay = true;
                    }
                    KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.theme_search_query.clear();
                        self.theme_picker_state.select(Some(0));
                        self.show_theme_picker = true;
                    }
                    KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        if let Ok(mut guard) = self.agent_loop.try_lock() {
                            guard.session_tree = pi_session::SessionTree::new();
                            self.history.clear();
                            self.estimated_tokens = 0;
                            self.context_pct = 0.0;
                            self.history.push(("system".to_string(), "Started new session.".to_string()));
                        }
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        if self.input_text.is_empty() {
                            if self.is_agent_running {
                                self.abort_active_turn();
                            }
                            self.is_running = false;
                        } else {
                            self.input_text.clear();
                            self.cursor_pos = 0;
                            self.history_index = None;
                        }
                    }
                    KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.history.clear();
                        self.history.push(("system".to_string(), "Cleared terminal transcript.".to_string()));
                    }
                    KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        if self.input_text.is_empty() {
                            if self.is_agent_running {
                                self.abort_active_turn();
                            }
                            self.is_running = false;
                        }
                    }
                    KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.model_search_query.clear();
                        self.model_picker_state.select(Some(0));
                        self.show_model_picker = true;
                    }
                    KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.provider_search_query.clear();
                        self.provider_picker_state.select(Some(0));
                        self.show_provider_picker = true;
                    }
                    KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.provider_search_query.clear();
                        self.provider_picker_state.select(Some(0));
                        self.show_provider_picker = true;
                    }
                    KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        let tx = self.event_tx.clone();
                        self.history.push(("system".to_string(), "Refreshing model catalogs in background...".to_string()));
                        tokio::spawn(async move {
                            let models = pi_providers::ModelCatalogLoader::fetch_all_models(true).await;
                            let _ = tx.send(AgentTaskEvent::ModelsRefreshed(models));
                        });
                    }
                    KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.expand_tools = !self.expand_tools;
                    }
                    KeyCode::PageUp => {
                        self.scroll_offset = self.scroll_offset.saturating_add(5);
                    }
                    KeyCode::PageDown => {
                        self.scroll_offset = self.scroll_offset.saturating_sub(5);
                    }
                    KeyCode::Left => {
                        self.cursor_pos = self.cursor_pos.saturating_sub(1);
                    }
                    KeyCode::Right => {
                        self.cursor_pos = (self.cursor_pos + 1).min(self.input_text.chars().count());
                    }
                    KeyCode::Home => {
                        self.cursor_pos = 0;
                    }
                    KeyCode::End => {
                        self.cursor_pos = self.input_text.chars().count();
                    }
                    KeyCode::Up => {
                        let suggestions = AutocompleteEngine::get_suggestions(&self.input_text);
                        if !suggestions.is_empty() {
                            self.autocomplete_selected = if self.autocomplete_selected == 0 {
                                suggestions.len().saturating_sub(1)
                            } else {
                                self.autocomplete_selected - 1
                            };
                        } else if !self.prompt_history.is_empty() {
                            let new_idx = match self.history_index {
                                None => self.prompt_history.len().saturating_sub(1),
                                Some(i) => i.saturating_sub(1),
                            };
                            self.history_index = Some(new_idx);
                            self.input_text = self.prompt_history[new_idx].clone();
                            self.cursor_pos = self.input_text.chars().count();
                        }
                    }
                    KeyCode::Down => {
                        let suggestions = AutocompleteEngine::get_suggestions(&self.input_text);
                        if !suggestions.is_empty() {
                            self.autocomplete_selected = if self.autocomplete_selected + 1 >= suggestions.len() {
                                0
                            } else {
                                self.autocomplete_selected + 1
                            };
                        } else if let Some(i) = self.history_index {
                            if i + 1 < self.prompt_history.len() {
                                let new_idx = i + 1;
                                self.history_index = Some(new_idx);
                                self.input_text = self.prompt_history[new_idx].clone();
                                self.cursor_pos = self.input_text.chars().count();
                            } else {
                                self.history_index = None;
                                self.input_text.clear();
                                self.cursor_pos = 0;
                            }
                        }
                    }
                    KeyCode::Tab => {
                        let suggestions = AutocompleteEngine::get_suggestions(&self.input_text);
                        if !suggestions.is_empty() {
                            let chosen = suggestions[self.autocomplete_selected.min(suggestions.len() - 1)].0;
                            if self.input_text.starts_with("/login ") || self.input_text.starts_with("/provider ") {
                                let prefix = self.input_text.split_whitespace().next().unwrap_or("/login");
                                self.input_text = format!("{} {} ", prefix, chosen);
                            } else if self.input_text.starts_with("/theme ") {
                                self.input_text = format!("/theme {} ", chosen);
                            } else {
                                self.input_text = format!("{} ", chosen);
                            }
                            self.cursor_pos = self.input_text.chars().count();
                            self.autocomplete_selected = 0;
                        }
                    }
                    KeyCode::Esc => {
                        if self.is_agent_running {
                            self.abort_active_turn();
                        } else {
                            self.history.push(("system".to_string(), "Interrupted active execution.".to_string()));
                        }
                    }
                    KeyCode::Enter => {
                        if key.modifiers.contains(KeyModifiers::SHIFT) || key.modifiers.contains(KeyModifiers::CONTROL) {
                            let mut chars: Vec<char> = self.input_text.chars().collect();
                            if self.cursor_pos >= chars.len() {
                                chars.push('\n');
                            } else {
                                chars.insert(self.cursor_pos, '\n');
                            }
                            self.input_text = chars.into_iter().collect();
                            self.cursor_pos += 1;
                        } else if key.modifiers.contains(KeyModifiers::ALT) {
                            let text = self.input_text.trim().to_string();
                            if !text.is_empty() {
                                self.queued_messages.push(text.clone());
                                self.history.push(("system".to_string(), format!("Queued message: {}", text)));
                                self.input_text.clear();
                                self.cursor_pos = 0;
                            }
                        } else {
                            let text = self.input_text.trim().to_string();
                            if !text.is_empty() {
                                self.prompt_history.push(text.clone());
                                self.history_index = None;
                                self.scroll_offset = 0;

                                if text.starts_with('/') {
                                    self.handle_slash_command(&text).await;
                                } else {
                                    // Start async agent turn (aborts any active execution if steering)
                                    self.start_agent_turn(text);
                                }

                                self.input_text.clear();
                                self.cursor_pos = 0;
                            }
                        }
                    }
                    KeyCode::Char(c) => {
                        let mut chars: Vec<char> = self.input_text.chars().collect();
                        if self.cursor_pos >= chars.len() {
                            chars.push(c);
                        } else {
                            chars.insert(self.cursor_pos, c);
                        }
                        self.input_text = chars.into_iter().collect();
                        self.cursor_pos += 1;
                        self.autocomplete_selected = 0;
                    }
                    KeyCode::Backspace => {
                        if self.cursor_pos > 0 {
                            let mut chars: Vec<char> = self.input_text.chars().collect();
                            if self.cursor_pos <= chars.len() {
                                chars.remove(self.cursor_pos - 1);
                                self.input_text = chars.into_iter().collect();
                                self.cursor_pos -= 1;
                                self.autocomplete_selected = 0;
                            }
                        }
                    }
                    KeyCode::Delete => {
                        let mut chars: Vec<char> = self.input_text.chars().collect();
                        if self.cursor_pos < chars.len() {
                            chars.remove(self.cursor_pos);
                            self.input_text = chars.into_iter().collect();
                            self.autocomplete_selected = 0;
                        }
                    }
                    _ => {}
                }
            }
        }

        if let Some(handle) = self.active_turn.take() {
            handle.abort();
        }
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pi_tui_app_creation_and_autodiscovery() {
        let app = PiTuiApp::new("kilo/deepseek-r1");
        assert_eq!(app.model_id, "kilo/deepseek-r1");
        assert!(!app.all_catalog_models.is_empty());
        assert!(!app.history.is_empty());
        assert!(!app.is_agent_running);
        assert_eq!(app.theme.kind, ThemeKind::DefaultPi);
    }

    #[tokio::test]
    async fn test_theme_slash_command_handling() {
        let mut app = PiTuiApp::new("kilo/deepseek-r1");
        assert_eq!(app.theme.kind, ThemeKind::DefaultPi);

        // Switch to Tokyo Night
        app.handle_slash_command("/theme tokyonight").await;
        assert_eq!(app.theme.kind, ThemeKind::TokyoNight);
        assert!(app.history.last().unwrap().1.contains("Switched theme to Tokyo Night"));

        // Switch to Gruvbox Dark
        app.handle_slash_command("/theme gruvbox").await;
        assert_eq!(app.theme.kind, ThemeKind::GruvboxDark);

        // Open Theme Picker when no arg is passed
        app.handle_slash_command("/theme").await;
        assert!(app.show_theme_picker);
    }

    #[tokio::test]
    async fn test_help_slash_command_handling() {
        let mut app = PiTuiApp::new("kilo/deepseek-r1");
        assert!(!app.show_help_overlay);

        app.handle_slash_command("/help").await;
        assert!(app.show_help_overlay);
    }

    #[tokio::test]
    async fn test_token_progress_bar_rendering() {
        let mut app = PiTuiApp::new("kilo/deepseek-r1");
        app.context_pct = 42.0;

        let bar_line = app.render_token_progress_bar(16);
        let rendered_text = bar_line.spans.iter().map(|s| s.content.as_ref()).collect::<String>();
        assert!(rendered_text.contains("42% / 128k"));
        assert!(rendered_text.contains("█"));
        assert!(rendered_text.contains("░"));
    }

    #[tokio::test]
    async fn test_streaming_event_handling() {
        let mut app = PiTuiApp::new("kilo/deepseek-r1");

        app.event_tx.send(AgentTaskEvent::Event(TurnEvent::ModelStreaming {
            chunk: "Hello ".to_string(),
        })).unwrap();
        app.event_tx.send(AgentTaskEvent::Event(TurnEvent::ModelStreaming {
            chunk: "World!".to_string(),
        })).unwrap();

        app.poll_agent_events();

        let last = app.history.last().unwrap();
        assert_eq!(last.0, "pi");
        assert_eq!(last.1, "Hello World!");
    }

    #[tokio::test]
    async fn test_tool_event_handling() {
        let mut app = PiTuiApp::new("kilo/deepseek-r1");

        app.event_tx.send(AgentTaskEvent::Event(TurnEvent::ToolExecuting {
            tool_name: "read".to_string(),
            tool_call_id: "call_1".to_string(),
        })).unwrap();
        app.event_tx.send(AgentTaskEvent::Event(TurnEvent::ToolCompleted {
            tool_name: "read".to_string(),
            is_error: false,
        })).unwrap();

        app.poll_agent_events();

        let history_strs: Vec<String> = app.history.iter().map(|(r, m)| format!("{}: {}", r, m)).collect();
        assert!(history_strs.iter().any(|h| h.contains("Executing tool [read]")));
        assert!(history_strs.iter().any(|h| h.contains("Tool [read] completed")));
    }

    #[tokio::test]
    async fn test_interruption_handling() {
        let mut app = PiTuiApp::new("kilo/deepseek-r1");
        app.is_agent_running = true;

        app.abort_active_turn();
        assert!(!app.is_agent_running);
        assert_eq!(app.history.last().unwrap().1, "Interrupted active execution.");
    }

    #[tokio::test]
    async fn test_diff_slash_command_handling() {
        let mut app = PiTuiApp::new("kilo/deepseek-r1");
        assert!(!app.show_diff_overlay);
        assert!(app.diff_view_state.is_none());

        // Test /diff with existing file (e.g. Cargo.toml)
        app.handle_slash_command("/diff Cargo.toml").await;
        assert!(app.show_diff_overlay);
        assert!(app.diff_view_state.is_some());
        let state = app.diff_view_state.as_ref().unwrap();
        assert_eq!(state.file_path, "Cargo.toml");
        assert!(!state.lines.is_empty());

        // Test /diff with nonexistent file
        let mut app2 = PiTuiApp::new("kilo/deepseek-r1");
        app2.handle_slash_command("/diff non_existent_file_xyz_123.rs").await;
        assert!(!app2.show_diff_overlay);
        assert!(app2.history.last().unwrap().1.contains("File not found"));
    }
}
