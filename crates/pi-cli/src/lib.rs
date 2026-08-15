pub mod onboarding;

use anyhow::Result;
use clap::{CommandFactory, Parser};
use clap_complete::Shell;

#[derive(Parser, Debug)]
#[command(name = "tau")]
#[command(version, about = "Tau (τ) — High-Performance Autonomous Coding Agent (2π evolution of Pi)", long_about = None)]
pub struct Cli {
    /// One-shot query mode (prints output and exits)
    #[arg(short = 'p', long = "print")]
    pub print_query: Option<String>,

    /// Model ID to use (e.g. opencode/deepseek-v4-flash-free, anthropic/claude-3-7-sonnet-latest, openai/gpt-4o)
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
    #[arg(long = "init")]
    pub init: bool,
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
        print!("\nEnter API Key for [{}] (press Enter to skip): ", provider);
        use std::io::Write;
        std::io::stdout().flush()?;
        let mut key_input = String::new();
        std::io::stdin().read_line(&mut key_input)?;
        let trimmed = key_input.trim();
        if !trimmed.is_empty() {
            pi_providers::AuthResolver::save_key(&provider, trimmed)?;
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
        .unwrap_or_else(|| "opencode/deepseek-v4-flash-free".to_string());

    if let Some(query) = cli.print_query {
        println!("Running one-shot print mode query: {}", query);
        let model_cfg = pi_providers::ModelConfig::resolve(&active_model);
        let mut agent_loop = pi_core::AgentLoop::new(model_cfg);
        let res = agent_loop
            .run_turn(&query, |event| {
                println!("  [Event] {:?}", event);
            })
            .await?;
        println!("\n--- Result ---\n{}", res);
        return Ok(());
    }

    if let Some(ref replay_path) = cli.replay_session {
        let path = std::path::Path::new(replay_path);
        if !path.exists() {
            eprintln!("Error: Session file not found at '{}'", replay_path);
            std::process::exit(1);
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

            if cli.replay_delay_ms > 0 {
                tokio::time::sleep(tokio::time::Duration::from_millis(cli.replay_delay_ms)).await;
            }
        }

        println!("✓ Replay completed successfully ({} steps streamed).", trajectory.total_steps);
        return Ok(());
    }

    if cli.rpc_mode {
        eprintln!("Starting RPC mode over stdin/stdout...");
        pi_rpc::RpcServer::run_stdin_stdout_loop().await?;
        return Ok(());
    }

    // Default: Check first run without arguments and prompt onboarding
    if std::env::args().len() == 1 && !onboarding::config_exists() {
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
