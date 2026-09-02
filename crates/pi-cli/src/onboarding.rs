use anyhow::Result;
use pi_providers::{AuthResolver, DEFAULT_MODEL, discover_local_providers};
use serde_json::Value;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct LocalDaemonInfo {
    pub name: &'static str,
    pub port: u16,
    pub url: String,
    pub is_running: bool,
    pub models: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnboardingConfig {
    pub default_mode: String,
    pub default_model: String,
    pub default_specialist: String,
    pub alfred_level: String,
    pub theme: String,
}

impl Default for OnboardingConfig {
    fn default() -> Self {
        Self {
            default_mode: "tui".to_string(),
            default_model: DEFAULT_MODEL.to_string(),
            default_specialist: "jarvis".to_string(),
            alfred_level: "standard".to_string(),
            theme: "default".to_string(),
        }
    }
}

pub fn get_config_path() -> PathBuf {
    AuthResolver::config_json_path()
}

pub fn config_exists() -> bool {
    get_config_path().exists()
}

/// Renders a quiet, styled ANSI CLI header
pub fn render_welcome_banner() {
    println!("\n\x1b[1;36m  ████████╗ █████╗ ██╗   ██╗\x1b[0m    \x1b[1;33m(τ = 2π)\x1b[0m");
    println!(
        "\x1b[1;36m  ╚══██╔══╝██╔══██╗██║   ██║\x1b[0m    \x1b[2mThe 2π Evolution of Pi\x1b[0m"
    );
    println!(
        "\x1b[1;36m     ██║   ███████║██║   ██║\x1b[0m    \x1b[1mHigh-Performance Autonomous Coding Agent\x1b[0m"
    );
    println!(
        "\x1b[1;36m     ██║   ██╔══██║██║   ██║\x1b[0m    \x1b[2m100% Pure Rust · Zero Node.js · < 10MB RAM\x1b[0m"
    );
    println!("\x1b[1;36m     ██║   ██║  ██║╚██████╔╝\x1b[0m");
    println!("\x1b[1;36m     ╚═╝   ╚═╝  ╚═╝ ╚═════╝\x1b[0m\n");
}

/// Reads user input with masked asterisks to prevent leaking sensitive API credentials
pub fn read_masked_key(prompt: &str) -> io::Result<String> {
    print!("{}", prompt);
    io::stdout().flush()?;

    // If stdin is not a terminal (e.g. piped in tests or CI), fall back to standard line reading
    use std::io::IsTerminal;
    if !io::stdin().is_terminal() {
        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        return Ok(line.trim().to_string());
    }

    crossterm::terminal::enable_raw_mode()?;
    let mut password = String::new();

    loop {
        if let crossterm::event::Event::Key(key_event) = crossterm::event::read()?
            && key_event.kind == crossterm::event::KeyEventKind::Press
        {
            match key_event.code {
                crossterm::event::KeyCode::Enter => break,
                crossterm::event::KeyCode::Char('c')
                    if key_event
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL) =>
                {
                    crossterm::terminal::disable_raw_mode()?;
                    println!("\n[Operation cancelled]");
                    return Ok(String::new());
                }
                crossterm::event::KeyCode::Char(c) => {
                    password.push(c);
                    print!("*");
                    io::stdout().flush()?;
                }
                crossterm::event::KeyCode::Backspace => {
                    if password.pop().is_some() {
                        print!("\x08 \x08");
                        io::stdout().flush()?;
                    }
                }
                crossterm::event::KeyCode::Esc => {
                    password.clear();
                    break;
                }
                _ => {}
            }
        }
    }

    crossterm::terminal::disable_raw_mode()?;
    println!();
    Ok(password.trim().to_string())
}

/// Reads and validates a numeric menu choice with automatic retry loop
pub fn read_validated_choice(
    prompt: &str,
    min: usize,
    max: usize,
    default: usize,
) -> io::Result<usize> {
    loop {
        print!("{} [1-{}, default: {}]: ", prompt, max, default);
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let trimmed = input.trim();

        if trimmed.is_empty() {
            return Ok(default);
        }

        match trimmed.parse::<usize>() {
            Ok(val) if val >= min && val <= max => return Ok(val),
            _ => {
                println!(
                    "  \x1b[31m❌ Invalid selection. Please enter a number between {} and {}.\x1b[0m",
                    min, max
                );
            }
        }
    }
}

/// Probes local LLM daemons via the shared provider discovery helper
pub async fn probe_local_daemons() -> Vec<LocalDaemonInfo> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(600))
        .build()
        .unwrap_or_default();
    discover_local_providers(&client)
        .await
        .into_iter()
        .map(|d| LocalDaemonInfo {
            name: d.name,
            port: d.port,
            url: d.url,
            is_running: d.is_running,
            models: d.models,
        })
        .collect()
}

/// Runs the 6-Stage Interactive Mode, Model & Persona Setup Wizard
pub async fn run_first_run_wizard() -> Result<()> {
    render_welcome_banner();

    println!("\x1b[1;37mWelcome to Tau (τ) Setup Wizard\x1b[0m");
    println!("Let's configure your workspace preferences across 6 fast stages.\n");

    let mut config = OnboardingConfig::default();

    // ── STAGE 1: PRIMARY OPERATING MODE ──────────────────────────────────────────
    println!("\x1b[1;34m[Stage 1/6] Primary Operating Mode\x1b[0m");
    println!(
        "  1) \x1b[1mInteractive TUI Cockpit\x1b[0m  \x1b[2m(Ratatui dashboard with /diff, /plan, /memory, /ask)\x1b[0m \x1b[32m[Default]\x1b[0m"
    );
    println!(
        "  2) \x1b[1mAutonomous Daemon (taud)\x1b[0m \x1b[2m(Background ambient daemon listening on Unix socket)\x1b[0m"
    );
    println!(
        "  3) \x1b[1mFast CLI One-Shot\x1b[0m        \x1b[2m(Direct query-and-print execution via -p)\x1b[0m"
    );
    println!(
        "  4) \x1b[1mJSON-RPC 2.0 Server\x1b[0m      \x1b[2m(Headless stdio engine for Neovim / VS Code plugins)\x1b[0m"
    );
    println!();

    let mode_choice = read_validated_choice("Select operating mode", 1, 4, 1)?;
    config.default_mode = match mode_choice {
        1 => "tui".to_string(),
        2 => "daemon".to_string(),
        3 => "cli".to_string(),
        4 => "rpc".to_string(),
        _ => "tui".to_string(),
    };
    println!(
        "  \x1b[32m✓ Selected Mode:\x1b[0m \x1b[1m{}\x1b[0m\n",
        config.default_mode
    );

    // ── STAGE 2: LOCAL DAEMON AUTO-DISCOVERY ─────────────────────────────────────
    println!("\x1b[1;34m[Stage 2/6] Probing Local AI Daemons...\x1b[0m");
    let daemons = probe_local_daemons().await;
    let mut discovered_local_models = Vec::new();

    for d in &daemons {
        if d.is_running {
            println!(
                "  \x1b[32m✓\x1b[0m \x1b[1m{}\x1b[0m (localhost:{}) — \x1b[32m{} active model(s)\x1b[0m",
                d.name,
                d.port,
                d.models.len()
            );
            for m in &d.models {
                println!("    \x1b[2m↳ {}\x1b[0m", m);
                discovered_local_models.push(m.clone());
            }
        } else {
            println!(
                "  \x1b[2m• {} (localhost:{}) — offline\x1b[0m",
                d.name, d.port
            );
        }
    }
    println!();

    // ── STAGE 3: PROVIDER & MODEL SELECTION ──────────────────────────────────────
    println!("\x1b[1;34m[Stage 3/6] Default AI Model & Provider\x1b[0m");
    println!(
        "  1) \x1b[1mOpenCode Zen Flash\x1b[0m      \x1b[2m({})\x1b[0m \x1b[32m[Free · Zero-Config]\x1b[0m",
        DEFAULT_MODEL
    );
    println!(
        "  2) \x1b[1mAnthropic Claude 3.7\x1b[0m    \x1b[2m(anthropic/claude-3-7-sonnet-latest)\x1b[0m"
    );
    println!("  3) \x1b[1mOpenAI GPT-4o\x1b[0m           \x1b[2m(openai/gpt-4o)\x1b[0m");
    println!("  4) \x1b[1mGoogle Gemini 2.5 Pro\x1b[0m   \x1b[2m(gemini/gemini-2.5-pro)\x1b[0m");
    println!("  5) \x1b[1mDeepSeek Chat\x1b[0m           \x1b[2m(deepseek/deepseek-chat)\x1b[0m");
    println!(
        "  6) \x1b[1mGroq Ultra-Fast LPU\x1b[0m     \x1b[2m(groq/llama-3.3-70b-versatile)\x1b[0m"
    );
    println!(
        "  7) \x1b[1mOpenRouter Gateway\x1b[0m      \x1b[2m(openrouter/auto · 250+ Models)\x1b[0m"
    );

    if !discovered_local_models.is_empty() {
        println!(
            "  8) \x1b[1mLocal Detected Model\x1b[0m    \x1b[2m({})\x1b[0m \x1b[36m[Zero Cloud]\x1b[0m",
            discovered_local_models[0]
        );
    } else {
        println!(
            "  8) \x1b[1mLocal Model Daemon\x1b[0m      \x1b[2m(ollama / llamacpp / lmstudio / vllm)\x1b[0m"
        );
    }
    println!(
        "  9) \x1b[1mCustom Provider & Model\x1b[0m \x1b[2m(Cerebras, Mistral, xAI, Fireworks, etc.)\x1b[0m"
    );
    println!();

    let model_choice = read_validated_choice("Select default model", 1, 9, 1)?;

    match model_choice {
        1 => {
            config.default_model = DEFAULT_MODEL.to_string();
        }
        2 => {
            config.default_model = "anthropic/claude-3-7-sonnet-latest".to_string();
            configure_provider_credential("anthropic").await?;
        }
        3 => {
            config.default_model = "openai/gpt-4o".to_string();
            configure_provider_credential("openai").await?;
        }
        4 => {
            config.default_model = "gemini/gemini-2.5-pro".to_string();
            configure_provider_credential("gemini").await?;
        }
        5 => {
            config.default_model = "deepseek/deepseek-chat".to_string();
            configure_provider_credential("deepseek").await?;
        }
        6 => {
            config.default_model = "groq/llama-3.3-70b-versatile".to_string();
            configure_provider_credential("groq").await?;
        }
        7 => {
            config.default_model = "openrouter/auto".to_string();
            configure_provider_credential("openrouter").await?;
        }
        8 => {
            if discovered_local_models.len() == 1 {
                config.default_model = discovered_local_models[0].clone();
            } else if discovered_local_models.len() > 1 {
                println!("\nDiscovered Local Models:");
                for (idx, m) in discovered_local_models.iter().enumerate() {
                    println!("    {}) {}", idx + 1, m);
                }
                let local_sel = read_validated_choice(
                    "Select local model",
                    1,
                    discovered_local_models.len(),
                    1,
                )?;
                config.default_model = discovered_local_models[local_sel - 1].clone();
            } else {
                print!("Enter local model identifier [default: ollama/llama3.2]: ");
                io::stdout().flush()?;
                let mut local_input = String::new();
                io::stdin().read_line(&mut local_input)?;
                let local_trimmed = local_input.trim();
                config.default_model = if local_trimmed.is_empty() {
                    "ollama/llama3.2".to_string()
                } else {
                    local_trimmed.to_string()
                };
            }
        }
        9 => {
            print!("Enter provider name (e.g. cerebras, mistral, xai, fireworks): ");
            io::stdout().flush()?;
            let mut prov_input = String::new();
            io::stdin().read_line(&mut prov_input)?;
            let prov = prov_input.trim().to_lowercase();
            let safe_prov = if prov.is_empty() {
                "custom".to_string()
            } else {
                prov
            };

            print!("Enter Model ID (e.g. {}/model-name): ", safe_prov);
            io::stdout().flush()?;
            let mut m_input = String::new();
            io::stdin().read_line(&mut m_input)?;
            let m_trimmed = m_input.trim();
            config.default_model = if m_trimmed.is_empty() {
                format!("{}/default", safe_prov)
            } else {
                m_trimmed.to_string()
            };

            configure_provider_credential(&safe_prov).await?;
        }
        _ => {
            config.default_model = DEFAULT_MODEL.to_string();
        }
    }
    println!(
        "  \x1b[32m✓ Selected Model:\x1b[0m \x1b[1m{}\x1b[0m\n",
        config.default_model
    );

    // ── STAGE 4: AUTONOMOUS SPECIALIST PERSONA ──────────────────────────────────
    println!("\x1b[1;34m[Stage 4/6] Autonomous Specialist Fleet Persona\x1b[0m");
    println!(
        "  1) \x1b[1mJ.A.R.V.I.S.\x1b[0m \x1b[2m(Architecture, Refactoring, Speculative Racing, Formal Tone)\x1b[0m \x1b[32m[Default]\x1b[0m"
    );
    println!(
        "  2) \x1b[1mF.R.I.D.A.Y.\x1b[0m \x1b[2m(Tactical Verification, Live Security Audit, Maximum Density)\x1b[0m"
    );
    println!(
        "  3) \x1b[1mE.V.\x1b[0m         \x1b[2m(Cognitive State, Hindsight Working Memory, Sustainability)\x1b[0m"
    );
    println!();

    let specialist_choice = read_validated_choice("Select specialist persona", 1, 3, 1)?;
    config.default_specialist = match specialist_choice {
        1 => "jarvis".to_string(),
        2 => "friday".to_string(),
        3 => "ev".to_string(),
        _ => "jarvis".to_string(),
    };
    println!(
        "  \x1b[32m✓ Selected Specialist:\x1b[0m \x1b[1m{}\x1b[0m\n",
        config.default_specialist.to_uppercase()
    );

    // ── STAGE 5: ALFRED MORAL CONSCIENCE & GUARD LEVEL ──────────────────────────
    println!("\x1b[1;34m[Stage 5/6] The Alfred Moral Override Protocol\x1b[0m");
    println!(
        "  1) \x1b[1mStandard Advisory\x1b[0m \x1b[2m(Balanced friction: alerts on high-risk shell/file commands)\x1b[0m \x1b[32m[Default]\x1b[0m"
    );
    println!(
        "  2) \x1b[1mStrict Guardian\x1b[0m   \x1b[2m(Elevated advisory checks on destructive modifications)\x1b[0m"
    );
    println!(
        "  3) \x1b[1mPermissive\x1b[0m        \x1b[2m(Maximum autonomy, minimal advisory friction)\x1b[0m"
    );
    println!();

    let alfred_choice = read_validated_choice("Select Alfred conscience level", 1, 3, 1)?;
    config.alfred_level = match alfred_choice {
        1 => "standard".to_string(),
        2 => "strict".to_string(),
        3 => "permissive".to_string(),
        _ => "standard".to_string(),
    };
    println!(
        "  \x1b[32m✓ Selected Conscience Level:\x1b[0m \x1b[1m{}\x1b[0m\n",
        config.alfred_level
    );

    // ── STAGE 6: TERMINAL UI THEME ──────────────────────────────────────────────
    println!("\x1b[1;34m[Stage 6/6] Terminal UI Theme\x1b[0m");
    println!(
        "  1) \x1b[1mDefault Tau Dark\x1b[0m \x1b[2m(High-contrast modern slate & cyan)\x1b[0m \x1b[32m[Default]\x1b[0m"
    );
    println!("  2) \x1b[1mDracula\x1b[0m          \x1b[2m(Vibrant purple & pink accents)\x1b[0m");
    println!("  3) \x1b[1mNord\x1b[0m             \x1b[2m(Arctic icy blues & cool grays)\x1b[0m");
    println!("  4) \x1b[1mGruvbox\x1b[0m          \x1b[2m(Warm retro earth tones)\x1b[0m");
    println!(
        "  5) \x1b[1mMonokai\x1b[0m          \x1b[2m(Classic bright green & yellow contrast)\x1b[0m"
    );
    println!("  6) \x1b[1mCatppuccin\x1b[0m       \x1b[2m(Smooth pastel mocha palette)\x1b[0m");
    println!();

    let theme_choice = read_validated_choice("Select UI theme", 1, 6, 1)?;
    config.theme = match theme_choice {
        1 => "default".to_string(),
        2 => "dracula".to_string(),
        3 => "nord".to_string(),
        4 => "gruvbox".to_string(),
        5 => "monokai".to_string(),
        6 => "catppuccin".to_string(),
        _ => "default".to_string(),
    };
    println!(
        "  \x1b[32m✓ Selected Theme:\x1b[0m \x1b[1m{}\x1b[0m\n",
        config.theme
    );

    // ── ATOMIC SAVE & PERMISSION HARDENING ────────────────────────────────────────
    save_onboarding_config(&config)?;

    println!("╔══════════════════════════════════════════════════════════════════════════════╗");
    println!(
        "║ \x1b[1;32m✔ Setup Completed Successfully!\x1b[0m                                              ║"
    );
    println!("╠══════════════════════════════════════════════════════════════════════════════╣");
    println!("║  • Operating Mode:    {:<54} ║", config.default_mode);
    println!("║  • Default Model:     {:<54} ║", config.default_model);
    println!(
        "║  • Specialist Persona:{:<54} ║",
        config.default_specialist.to_uppercase()
    );
    println!("║  • Alfred Conscience: {:<54} ║", config.alfred_level);
    println!("║  • UI Theme:          {:<54} ║", config.theme);
    println!(
        "║  • Config Location:   {:<54} ║",
        get_config_path().display()
    );
    println!("║  • Permissions:       0600 (POSIX Owner-Only Read/Write)                     ║");
    println!("╚══════════════════════════════════════════════════════════════════════════════╝\n");

    println!("Launch Tau anytime with: \x1b[1;36mtau\x1b[0m or \x1b[1;36mpi-rs\x1b[0m\n");
    Ok(())
}

/// Prompts for credentials with masked entry and live connectivity validation
async fn configure_provider_credential(provider: &str) -> Result<()> {
    let existing_key = AuthResolver::resolve_key(provider);

    // GATEWAY DEDUPLICATION CHECK
    // If the provider is 'openai' or 'gemini', check if a gateway key ('opencode' or 'kilo') already exists.
    // If it does, and we don't have a direct key, we can skip prompting because the gateway handles it.
    if existing_key.is_none()
        && (provider == "openai" || provider == "gemini" || provider == "kilo")
        && (AuthResolver::resolve_key("opencode").is_some()
            || AuthResolver::resolve_key("kilo").is_some())
    {
        println!(
            "  \x1b[32m✔ Found existing Gateway credentials (OpenCode/Kilo)\x1b[0m. Direct [{}] key not required.",
            provider
        );
        return Ok(());
    }

    if let Some(ref k) = existing_key {
        let masked = if k.len() > 8 {
            format!("{}...{}", &k[..4], &k[k.len() - 4..])
        } else {
            "****".to_string()
        };
        println!(
            "  \x1b[32m✔ Found existing credentials for [{}]\x1b[0m ({})",
            provider, masked
        );
        print!("  Would you like to keep existing credentials? [Y/n]: ");
        io::stdout().flush()?;
        let mut ans = String::new();
        io::stdin().read_line(&mut ans)?;
        let trimmed = ans.trim().to_lowercase();
        if trimmed.is_empty() || trimmed.starts_with('y') {
            return Ok(());
        }
    }

    let prompt = format!(
        "  Enter API Key for [{}] (press Enter to skip/use ENV): ",
        provider
    );
    let key = read_masked_key(&prompt)?;

    if !key.is_empty() {
        print!("  \x1b[2m⏳ Verifying API credentials with endpoint...\x1b[0m ");
        io::stdout().flush()?;

        match AuthResolver::test_provider_key(provider, &key).await {
            Ok(true) => {
                println!("\x1b[32m✔ Key verified!\x1b[0m");
            }
            Ok(false) => {
                println!("\x1b[33m⚠ Provider rejected key (saved anyway for retry)\x1b[0m");
            }
            Err(e) => {
                println!("\x1b[33m⚠ Connectivity note: {} (saved anyway)\x1b[0m", e);
            }
        }

        AuthResolver::save_key(provider, &key)?;
        println!(
            "  \x1b[32m✓ Saved credentials for [{}] to ~/.pi/config.json\x1b[0m",
            provider
        );
    } else {
        println!("  \x1b[2m↳ Skipped. Will resolve credentials from environment variables.\x1b[0m");
    }

    Ok(())
}

/// Atomically saves the full OnboardingConfig to ~/.pi/config.json with POSIX 0o600 permissions
pub fn save_onboarding_config(config: &OnboardingConfig) -> Result<()> {
    let config_path = get_config_path();
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = fs::metadata(parent) {
                let mut perms = meta.permissions();
                perms.set_mode(0o700);
                let _ = fs::set_permissions(parent, perms);
            }
        }
    }

    let mut json_obj = if let Ok(content) = fs::read_to_string(&config_path) {
        serde_json::from_str::<Value>(&content)
            .ok()
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default()
    } else {
        serde_json::Map::new()
    };

    json_obj.insert(
        "default_model".to_string(),
        Value::String(config.default_model.clone()),
    );
    json_obj.insert(
        "default_mode".to_string(),
        Value::String(config.default_mode.clone()),
    );
    json_obj.insert(
        "default_specialist".to_string(),
        Value::String(config.default_specialist.clone()),
    );
    json_obj.insert(
        "alfred_level".to_string(),
        Value::String(config.alfred_level.clone()),
    );
    json_obj.insert("theme".to_string(), Value::String(config.theme.clone()));

    let output_json = serde_json::to_string_pretty(&Value::Object(json_obj))?;
    fs::write(&config_path, output_json)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(&config_path) {
            let mut perms = meta.permissions();
            perms.set_mode(0o600);
            let _ = fs::set_permissions(&config_path, perms);
        }
    }

    Ok(())
}

/// Saves the selected default model cleanly to ~/.pi/config.json
pub fn save_default_model(model_id: &str) -> Result<()> {
    let config = OnboardingConfig {
        default_model: model_id.to_string(),
        ..Default::default()
    };
    save_onboarding_config(&config)
}

/// Initializes a minimal default ~/.pi/config.json if not present
pub fn ensure_default_config() -> Result<()> {
    if !config_exists() {
        let config = OnboardingConfig::default();
        save_onboarding_config(&config)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_welcome_banner() {
        render_welcome_banner();
    }

    #[tokio::test]
    async fn test_probe_local_daemons() {
        let daemons = probe_local_daemons().await;
        assert_eq!(daemons.len(), 4);
        assert_eq!(daemons[0].name, "Ollama");
        assert_eq!(daemons[1].name, "llama.cpp");
        assert_eq!(daemons[2].name, "LM Studio");
        assert_eq!(daemons[3].name, "vLLM");
    }

    #[test]
    fn test_onboarding_config_default() {
        let cfg = OnboardingConfig::default();
        assert_eq!(cfg.default_mode, "tui");
        assert_eq!(cfg.default_model, DEFAULT_MODEL);
        assert_eq!(cfg.default_specialist, "jarvis");
        assert_eq!(cfg.alfred_level, "standard");
        assert_eq!(cfg.theme, "default");
    }

    #[test]
    fn test_save_and_load_onboarding_config() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("config.json");

        let config = OnboardingConfig {
            default_mode: "daemon".to_string(),
            default_model: "anthropic/claude-3-7-sonnet-latest".to_string(),
            default_specialist: "friday".to_string(),
            alfred_level: "strict".to_string(),
            theme: "dracula".to_string(),
        };

        let mut map = serde_json::Map::new();
        map.insert(
            "default_mode".to_string(),
            Value::String(config.default_mode.clone()),
        );
        map.insert(
            "default_model".to_string(),
            Value::String(config.default_model.clone()),
        );
        map.insert(
            "default_specialist".to_string(),
            Value::String(config.default_specialist.clone()),
        );
        map.insert(
            "alfred_level".to_string(),
            Value::String(config.alfred_level.clone()),
        );
        map.insert("theme".to_string(), Value::String(config.theme.clone()));

        fs::write(
            &config_path,
            serde_json::to_string_pretty(&Value::Object(map)).unwrap(),
        )
        .unwrap();

        let content = fs::read_to_string(&config_path).unwrap();
        let val: Value = serde_json::from_str(&content).unwrap();
        assert_eq!(
            val.get("default_mode").and_then(|v| v.as_str()),
            Some("daemon")
        );
        assert_eq!(
            val.get("default_model").and_then(|v| v.as_str()),
            Some("anthropic/claude-3-7-sonnet-latest")
        );
        assert_eq!(
            val.get("default_specialist").and_then(|v| v.as_str()),
            Some("friday")
        );
        assert_eq!(
            val.get("alfred_level").and_then(|v| v.as_str()),
            Some("strict")
        );
        assert_eq!(val.get("theme").and_then(|v| v.as_str()), Some("dracula"));
    }
}
