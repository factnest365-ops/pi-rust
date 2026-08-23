pub mod onboarding;

use anyhow::Result;
use clap::{CommandFactory, Parser};
use clap_complete::Shell;

use pi_providers::DEFAULT_MODEL;

#[derive(Parser, Debug)]
#[command(name = "tau")]
#[command(version, about = "Tau (τ) — High-Performance Autonomous Coding Agent (2π evolution of Pi)", long_about = None)]
pub struct Cli {
    /// One-shot query mode (prints output and exits)
    #[arg(short = 'p', long = "print")]
    pub print_query: Option<String>,

    /// Model ID to use (e.g. opencode/{}, anthropic/claude-3-7-sonnet-latest, openai/gpt-4o)
    #[arg(short = 'm', long = "model")]
    pub model: Option<String>,

    /// RPC mode over stdin/stdout JSON
    #[arg(long = "rpc")]
    pub rpc_mode: bool,

    /// List all discovered models and exits
    #[arg(short = 'M', long = "models")]
    pub list_models: bool,

    /// Force refresh online model catalogs & local daemons, then list all models
    #[arg(long = "refresh-models")]
    pub refresh_models: bool,

    /// Configure authentication credentials for a provider (e.g. anthropic, openai, gemini, kilo, openrouter)
    #[arg(long = "login")]
    pub login_provider: Option<Option<String>>,

    /// Replay a stored JSONL session file to stdout
    #[arg(long = "replay")]
    pub replay_session: Option<String>,

    /// Milliseconds delay between steps during session replay (default: 50)
    #[arg(long = "replay-delay-ms", default_value = "50")]
    pub replay_delay_ms: u64,

    /// Generate shell completions for the specified shell (bash, zsh, fish, powershell, elvish)
    #[arg(long = "completions", value_enum)]
    pub completions: Option<Shell>,

    /// Run the interactive first-run onboarding wizard
    #[arg(long = "init", visible_aliases = ["setup", "wizard"])]
    pub init: bool,

    /// Query the status of the running background daemon (taud)
    #[arg(long = "daemon-status")]
    pub daemon_status: bool,

    /// Ping the background daemon (taud) over Unix socket
    #[arg(long = "daemon-ping")]
    pub daemon_ping: bool,

    /// Select active specialist persona (jarvis, friday, ev)
    #[arg(short = 's', long = "specialist")]
    pub specialist: Option<String>,

    /// Undo the last recorded action snapshot
    #[arg(long = "undo")]
    pub undo: bool,

    /// Evaluate an action against the Alfred Moral Override Protocol
    #[arg(long = "alfred-check")]
    pub alfred_check: Option<String>,
}

pub async fn run_cli() -> Result<()> {
    let cli = Cli::parse();

    if let Some(shell) = cli.completions {
        let mut cmd = Cli::command();
        clap_complete::generate(shell, &mut cmd, "tau", &mut std::io::stdout());
        return Ok(());
    }

    if cli.init {
        onboarding::run_first_run_wizard().await?;
        return Ok(());
    }

    if cli.daemon_ping {
        let client = pi_daemon::DaemonClient::default_client();
        if !client.is_daemon_running() {
            println!("❌ taud daemon is not running (socket not found at ~/.tau/taud.sock).");
            println!("Launch with: cargo run -p pi-daemon --bin taud");
            return Ok(());
        }
        match client.ping().await {
            Ok(pong) => println!("✔ taud daemon alive: {}", pong),
            Err(e) => println!("❌ Error connecting to daemon: {}", e),
        }
        return Ok(());
    }

    if cli.daemon_status {
        let client = pi_daemon::DaemonClient::default_client();
        if !client.is_daemon_running() {
            println!("❌ taud daemon is not running (socket not found at ~/.tau/taud.sock).");
            return Ok(());
        }
        let status = client.status().await?;
        println!("\n=== Tau Background Daemon (taud) Status ===");
        println!("Version:              {}", status.version);
        println!("Uptime:               {}s", status.uptime_secs);
        println!("Active Specialist:    {}", status.active_specialist.display_name());
        println!("Memories in Vault:    {}", status.memory_count);
        println!("Reversible Actions:   {}", status.reversible_actions);
        println!("\nSpecialists:");
        for s in &status.specialists {
            println!("  • {:<12} — {}", s.name, s.role);
        }
        println!();
        return Ok(());
    }

    if cli.undo {
        let mut undo = pi_core::UndoEngine::new();
        println!("Rolling back last action...");
        match undo.undo_last(1) {
            Ok(msgs) => {
                for m in msgs {
                    println!("✔ {}", m);
                }
            }
            Err(e) => println!("ℹ {}", e),
        }
        return Ok(());
    }

    if let Some(ref goal) = cli.alfred_check {
        let mut alfred = pi_core::AlfredProtocol::new();
        println!("\n=== Alfred Moral Override Protocol Evaluation ===");
        println!("Proposed Action: '{}'\n", goal);
        if let Some(advisory) = alfred.evaluate_action(goal, "") {
            let (badge, _) = advisory.level.badge();
            println!("[{}]", badge);
            println!("Principle: {}", advisory.principle_text);
            println!("\nAdvisory Message:\n{}", advisory.advisory_message);
        } else {
            println!("✔ Action cleared. No core value conflicts detected.");
        }
        println!();
        return Ok(());
    }

    if cli.list_models || cli.refresh_models {
        if cli.refresh_models {
            println!("🔄 Querying online provider endpoints & local daemons...");
        }
        let models = pi_providers::ModelCatalogLoader::fetch_all_models(cli.refresh_models).await;
        println!("\n=== Available Models ({} Discovered) ===", models.len());
        println!("{:<32} {:<18} {:<10} {:<12} DESCRIPTION", "MODEL ID", "PROVIDER", "CONTEXT", "CAPABILITIES");
        println!("{}", "-".repeat(105));
        for m in &models {
            let ctx = if m.context_window >= 1_000_000 {
                format!("{}M", m.context_window / 1_000_000)
            } else {
                format!("{}k", m.context_window / 1_000)
            };
            let mut caps = Vec::new();
            if m.supports_reasoning {
                caps.push("Reasoning");
            }
            if m.supports_vision {
                caps.push("Vision");
            }
            let caps_str = if caps.is_empty() { "Standard".to_string() } else { caps.join("+") };
            println!("{:<32} {:<18} {:<10} {:<12} {}", m.id, m.provider, ctx, caps_str, m.description);
        }
        println!("\nUse in interactive TUI via: tau --model <model-id> or hotkey Ctrl+L\n");
        return Ok(());
    }

    if let Some(opt_prov) = cli.login_provider {
        let provider = opt_prov.unwrap_or_else(|| "anthropic".to_string());
        println!("\n=== Tau Authentication & Login ===");
        println!("{}", pi_providers::AuthResolver::get_login_instructions(&provider));
        let prompt = format!("\nEnter API Key for [{}] (press Enter to skip): ", provider);
        let trimmed = onboarding::read_masked_key(&prompt)?;
        if !trimmed.is_empty() {
            print!("⏳ Verifying API credentials with endpoint... ");
            use std::io::Write;
            std::io::stdout().flush()?;
            match pi_providers::AuthResolver::test_provider_key(&provider, &trimmed).await {
                Ok(true) => println!("\x1b[32m✔ Key verified!\x1b[0m"),
                Ok(false) => println!("\x1b[33m⚠ Provider rejected key (saved anyway)\x1b[0m"),
                Err(e) => println!("\x1b[33m⚠ Note: {} (saved anyway)\x1b[0m", e),
            }
            pi_providers::AuthResolver::save_key(&provider, &trimmed)?;
            println!("✓ Successfully saved credentials for [{}] to ~/.pi/config.json & ~/.pi/agent/auth.json\n", provider);
        } else {
            println!("No key entered. Login skipped.\n");
        }
        return Ok(());
    }

    // Resolve active model from CLI arg, config file default_model, or fallback
    let default_cfg_model = pi_providers::PiConfig::load().default_model;
    let active_model = cli
        .model
        .or(default_cfg_model)
        .unwrap_or_else(|| DEFAULT_MODEL.to_string());

    if let Some(query) = cli.print_query {
        let model_cfg = pi_providers::ModelConfig::resolve(&active_model);
        let mut agent_loop = pi_core::AgentLoop::new(model_cfg);
        let res = agent_loop
            .run_turn(&query, |event| {
                eprintln!("  [Event] {:?}", event);
            })
            .await?;
        println!("{}", res);
        return Ok(());
    }

    if let Some(ref replay_path) = cli.replay_session {
        replay_session_from_path(replay_path, cli.replay_delay_ms).await?;
        return Ok(());
    }

    if cli.rpc_mode {
        eprintln!("Starting RPC mode over stdin/stdout...");
        pi_rpc::RpcServer::run_stdin_stdout_loop(Some(&active_model)).await?;
        return Ok(());
    }

    // Default: Check first run without arguments in an interactive terminal and prompt onboarding
    use std::io::IsTerminal;
    if std::env::args().len() == 1 && std::io::stdin().is_terminal() && !onboarding::config_exists() {
        onboarding::render_welcome_banner();
        print!("Welcome to τ Tau! No configuration found at ~/.pi/config.json.\nWould you like to run the first-run onboarding wizard now? [Y/n]: ");
        use std::io::Write;
        std::io::stdout().flush()?;
        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer)?;
        let trimmed = answer.trim().to_lowercase();
        if trimmed.is_empty() || trimmed.starts_with('y') {
            onboarding::run_first_run_wizard().await?;
        } else {
            onboarding::ensure_default_config()?;
        }
    }

    // Run Interactive TUI
    let mut app = pi_tui::PiTuiApp::new(&active_model);
    app.run_loop().await?;

    Ok(())
}

/// Replays a stored JSONL session file to stdout with configured step delay
pub async fn replay_session_from_path(replay_path: &str, delay_ms: u64) -> Result<()> {
    let path = std::path::Path::new(replay_path);
    if !path.exists() {
        anyhow::bail!("Session file not found at '{}'", replay_path);
    }
    let tree = pi_session::SessionTree::load_from_jsonl(path)?;
    let trajectory = tree.export_trajectory(None);

    println!("\n╔════════════════════════════════════════════════════════════════════════════════╗");
    println!("║ 🎬 REPLAYING SESSION: {:<56} ║", trajectory.session_id);
    println!("║ Total Steps: {:<6} Estimated Tokens: {:<44} ║", trajectory.total_steps, trajectory.total_estimated_tokens);
    println!("╚════════════════════════════════════════════════════════════════════════════════╝\n");

    for step in &trajectory.steps {
        let role_label = match step.role {
            pi_session::Role::User => "\x1b[1;32m[USER]\x1b[0m",
            pi_session::Role::Assistant => "\x1b[1;36m[ASSISTANT]\x1b[0m",
            pi_session::Role::System => "\x1b[1;33m[SYSTEM]\x1b[0m",
            pi_session::Role::Tool => "\x1b[1;35m[TOOL OUTPUT]\x1b[0m",
        };

        println!("─ Step {} ── {} ({}) ─────────────────────────────", step.step_index + 1, role_label, step.timestamp);
        if let Some(ref tname) = step.tool_name {
            println!("  🔧 Executed Tool: \x1b[1m{}\x1b[0m", tname);
        }
        if let Some(ref tcalls) = step.tool_calls {
            println!("  🛠️  Tool Calls: {}", serde_json::to_string_pretty(tcalls).unwrap_or_default());
        }
        println!("{}\n", step.content);

        if delay_ms > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
        }
    }

    println!("✓ Replay completed successfully ({} steps streamed).", trajectory.total_steps);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parse_print_mode() {
        let args = ["tau", "-p", "Write a fibonacci function in Rust"];
        let cli = Cli::try_parse_from(args).expect("should parse -p");
        assert_eq!(cli.print_query, Some("Write a fibonacci function in Rust".to_string()));
        assert!(!cli.rpc_mode);
    }

    #[test]
    fn test_cli_parse_model_and_rpc() {
        let args = ["tau", "--rpc", "-m", "anthropic/claude-3-7-sonnet-latest"];
        let cli = Cli::try_parse_from(args).expect("should parse --rpc with --model");
        assert!(cli.rpc_mode);
        assert_eq!(cli.model, Some("anthropic/claude-3-7-sonnet-latest".to_string()));
    }

    #[test]
    fn test_cli_parse_replay_and_delay() {
        let args = ["tau", "--replay", "sample.jsonl", "--replay-delay-ms", "100"];
        let cli = Cli::try_parse_from(args).expect("should parse --replay");
        assert_eq!(cli.replay_session, Some("sample.jsonl".to_string()));
        assert_eq!(cli.replay_delay_ms, 100);
    }

    #[test]
    fn test_cli_parse_models_list_and_refresh() {
        let args = ["tau", "-M", "--refresh-models"];
        let cli = Cli::try_parse_from(args).expect("should parse -M and --refresh-models");
        assert!(cli.list_models);
        assert!(cli.refresh_models);
    }

    #[test]
    fn test_cli_parse_login_options() {
        let args = ["tau", "--login", "gemini"];
        let cli = Cli::try_parse_from(args).expect("should parse --login gemini");
        assert_eq!(cli.login_provider, Some(Some("gemini".to_string())));

        let args_bare = ["tau", "--login"];
        let cli_bare = Cli::try_parse_from(args_bare).expect("should parse bare --login");
        assert_eq!(cli_bare.login_provider, Some(None));
    }

    #[test]
    fn test_cli_parse_init_and_completions() {
        let args_init = ["tau", "--init"];
        let cli_init = Cli::try_parse_from(args_init).expect("should parse --init");
        assert!(cli_init.init);

        let args_setup = ["tau", "--setup"];
        let cli_setup = Cli::try_parse_from(args_setup).expect("should parse --setup");
        assert!(cli_setup.init);

        let args_wizard = ["tau", "--wizard"];
        let cli_wizard = Cli::try_parse_from(args_wizard).expect("should parse --wizard");
        assert!(cli_wizard.init);

        let args_comp = ["tau", "--completions", "zsh"];
        let cli_comp = Cli::try_parse_from(args_comp).expect("should parse --completions");
        assert_eq!(cli_comp.completions, Some(Shell::Zsh));
    }

    #[tokio::test]
    async fn test_replay_session_from_path_missing_file() {
        let result = replay_session_from_path("/non/existent/path/session.jsonl", 0).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Session file not found"));
    }

    #[tokio::test]
    async fn test_replay_session_from_path_valid_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let session_file = temp_dir.path().join("session.jsonl");

        let mut tree = pi_session::SessionTree::new_with_disk_path(
            Some(session_file.clone()),
            temp_dir.path().to_str().unwrap(),
            "test-replay-session".to_string(),
        );

        tree.append_child(pi_session::Role::User, "Hello assistant".to_string());
        tree.append_child(pi_session::Role::Assistant, "Hello user, how can I help?".to_string());

        let result = replay_session_from_path(session_file.to_str().unwrap(), 0).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_cli_parse_daemon_and_jarvis_flags() {
        let args = ["tau", "--daemon-ping", "--daemon-status", "-s", "jarvis", "--undo", "--alfred-check", "rm -rf /"];
        let cli = Cli::try_parse_from(args).expect("should parse daemon and jarvis flags");
        assert!(cli.daemon_ping);
        assert!(cli.daemon_status);
        assert_eq!(cli.specialist, Some("jarvis".to_string()));
        assert!(cli.undo);
        assert_eq!(cli.alfred_check, Some("rm -rf /".to_string()));
    }
}

