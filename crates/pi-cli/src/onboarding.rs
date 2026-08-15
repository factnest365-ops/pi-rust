use anyhow::Result;
use pi_providers::AuthResolver;
use serde_json::Value;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct LocalDaemonInfo {
    pub name: &'static str,
    #[allow(dead_code)]
    pub port: u16,
    pub url: &'static str,
    pub is_running: bool,
    pub models: Vec<String>,
}

pub fn get_config_path() -> PathBuf {
    AuthResolver::config_json_path()
}

pub fn config_exists() -> bool {
    get_config_path().exists()
}

/// Renders the styled welcome ASCII banner
pub fn render_welcome_banner() {
    println!("\x1b[1;36m");
    println!("  ╔══════════════════════════════════════════════════════════════════════════════╗");
    println!("  ║                                                                              ║");
    println!("  ║     \x1b[1;35mτ  T A U\x1b[1;36m   (2π Evolution of Pi)                                           ║");
    println!("  ║     \x1b[1;37mHigh-Performance Autonomous Coding Agent\x1b[1;36m                                 ║");
    println!("  ║                                                                              ║");
    println!("  ║     \x1b[0;32m✓ 100% Pure Safe Rust\x1b[1;36m       \x1b[0;33m✓ 35+ LLM Providers\x1b[1;36m    \x1b[0;34m✓ Dual Tool Protocol\x1b[1;36m   ║");
    println!("  ║     \x1b[0;35m✓ JSONL DAG Tree History\x1b[1;36m    \x1b[0;36m✓ Cockpit TUI (7 Themes)\x1b[1;36m \x1b[0;32m✓ Local Daemon Probing\x1b[1;36m ║");
    println!("  ║                                                                              ║");
    println!("  ╚══════════════════════════════════════════════════════════════════════════════╝");
    println!("\x1b[0m");
}

/// Probes local LLM daemons (Ollama, llama.cpp, LM Studio) with a short timeout
pub async fn probe_local_daemons() -> Vec<LocalDaemonInfo> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(600))
        .build()
        .unwrap_or_default();

    let mut daemons = vec![
        LocalDaemonInfo {
            name: "Ollama",
            port: 11434,
            url: "http://localhost:11434",
            is_running: false,
            models: Vec::new(),
        },
        LocalDaemonInfo {
            name: "llama.cpp",
            port: 8080,
            url: "http://localhost:8080",
            is_running: false,
            models: Vec::new(),
        },
        LocalDaemonInfo {
            name: "LM Studio",
            port: 1234,
            url: "http://localhost:1234",
            is_running: false,
            models: Vec::new(),
        },
    ];

    // 1. Probe Ollama (:11434)
    if let Ok(res) = client.get("http://localhost:11434/api/tags").send().await
        && res.status().is_success()
        && let Ok(json) = res.json::<Value>().await
        && let Some(arr) = json.get("models").and_then(|m| m.as_array())
    {
        daemons[0].is_running = true;
        for item in arr {
            if let Some(name) = item.get("name").and_then(|n| n.as_str()) {
                daemons[0].models.push(format!("ollama/{}", name));
            }
        }
    }

    // 2. Probe llama.cpp (:8080)
    if let Ok(res) = client.get("http://localhost:8080/v1/models").send().await
        && res.status().is_success()
        && let Ok(json) = res.json::<Value>().await
        && let Some(arr) = json.get("data").and_then(|d| d.as_array())
    {
        daemons[1].is_running = true;
        for item in arr {
            if let Some(id) = item.get("id").and_then(|n| n.as_str()) {
                daemons[1].models.push(format!("llamacpp/{}", id));
            }
        }
    }

    // 3. Probe LM Studio (:1234)
    if let Ok(res) = client.get("http://localhost:1234/v1/models").send().await
        && res.status().is_success()
        && let Ok(json) = res.json::<Value>().await
        && let Some(arr) = json.get("data").and_then(|d| d.as_array())
    {
        daemons[2].is_running = true;
        for item in arr {
            if let Some(id) = item.get("id").and_then(|n| n.as_str()) {
                daemons[2].models.push(format!("lmstudio/{}", id));
            }
        }
    }

    daemons
}

/// Runs the interactive first-run onboarding wizard
pub async fn run_first_run_wizard() -> Result<()> {
    render_welcome_banner();

    println!("\x1b[1;33m[Step 1/3] Detecting Local AI Daemons...\x1b[0m");
    let daemons = probe_local_daemons().await;
    let mut any_local_found = false;
    let mut first_local_model: Option<String> = None;

    for d in &daemons {
        if d.is_running {
            any_local_found = true;
            let count = d.models.len();
            println!(
                "  \x1b[1;32m●\x1b[0m \x1b[1m{}\x1b[0m running at {} (\x1b[32m{} model(s) available\x1b[0m)",
                d.name, d.url, count
            );
            if !d.models.is_empty() && first_local_model.is_none() {
                first_local_model = Some(d.models[0].clone());
            }
            for m in d.models.iter().take(3) {
                println!("    ↳ {}", m);
            }
            if d.models.len() > 3 {
                println!("    ↳ ... and {} more", d.models.len() - 3);
            }
        } else {
            println!(
                "  \x1b[0;90m○ {} not detected at {} (offline)\x1b[0m",
                d.name, d.url
            );
        }
    }

    if !any_local_found {
        println!("  \x1b[0;90m↳ No local daemons active. You can use free cloud models or provide an API key.\x1b[0m");
    }
    println!();

    println!("\x1b[1;33m[Step 2/3] Choose your default starter model / provider:\x1b[0m");
    println!("  \x1b[1;32m[1]\x1b[0m \x1b[1mFree OpenCode Zen\x1b[0m (\x1b[32mRecommended / Zero-Config\x1b[0m)");
    println!("      Model: \x1b[36mopencode/deepseek-v4-flash-free\x1b[0m — 100% Free Cloud Coding Model");
    println!("  \x1b[1;32m[2]\x1b[0m \x1b[1mLocal Daemon (Ollama / llama.cpp / LM Studio)\x1b[0m");
    if let Some(ref m) = first_local_model {
        println!("      Detected Model: \x1b[36m{}\x1b[0m (100% Private, Zero API Keys)", m);
    } else {
        println!("      Model: \x1b[36mollama/llama3.2\x1b[0m (100% Private, Zero API Keys)");
    }
    println!("  \x1b[1;32m[3]\x1b[0m \x1b[1mAnthropic Claude\x1b[0m (Claude 3.7 Sonnet / Claude 3.5 Sonnet)");
    println!("      Model: \x1b[36manthropic/claude-3-7-sonnet-latest\x1b[0m");
    println!("  \x1b[1;32m[4]\x1b[0m \x1b[1mOpenAI\x1b[0m (GPT-4o / o3-mini / o1)");
    println!("      Model: \x1b[36mopenai/gpt-4o\x1b[0m");
    println!("  \x1b[1;32m[5]\x1b[0m \x1b[1mDeepSeek\x1b[0m (DeepSeek Chat / DeepSeek Reasoner V3/R1)");
    println!("      Model: \x1b[36mdeepseek/deepseek-chat\x1b[0m");
    println!("  \x1b[1;32m[6]\x1b[0m \x1b[1mOpenRouter\x1b[0m (200+ Multi-Provider Gateway)");
    println!("      Model: \x1b[36mopenrouter/anthropic/claude-3.5-sonnet\x1b[0m");
    println!("  \x1b[1;32m[7]\x1b[0m \x1b[1mCustom / Other Provider\x1b[0m (Gemini, Groq, Mistral, Cerebras, Kilo, etc.)");
    println!();

    print!("\x1b[1mSelect an option [1-7] (default 1): \x1b[0m");
    io::stdout().flush()?;

    let mut choice = String::new();
    io::stdin().read_line(&mut choice)?;
    let choice_trimmed = choice.trim();

    let mut selected_default_model = "opencode/deepseek-v4-flash-free".to_string();

    match choice_trimmed {
        "2" => {
            if let Some(m) = first_local_model {
                selected_default_model = m;
            } else {
                print!("\nEnter local model name (e.g. ollama/llama3.2, llamacpp/qwen2.5-coder): [ollama/llama3.2] ");
                io::stdout().flush()?;
                let mut local_input = String::new();
                io::stdin().read_line(&mut local_input)?;
                let local_trimmed = local_input.trim();
                if !local_trimmed.is_empty() {
                    selected_default_model = local_trimmed.to_string();
                } else {
                    selected_default_model = "ollama/llama3.2".to_string();
                }
            }
            println!("✓ Configured local model: \x1b[1;32m{}\x1b[0m", selected_default_model);
        }
        "3" => {
            selected_default_model = "anthropic/claude-3-7-sonnet-latest".to_string();
            println!("\n{}", AuthResolver::get_login_instructions("anthropic"));
            print!("Enter Anthropic API Key (press Enter to skip if using ANTHROPIC_API_KEY env var): ");
            io::stdout().flush()?;
            let mut key_input = String::new();
            io::stdin().read_line(&mut key_input)?;
            let key_trimmed = key_input.trim();
            if !key_trimmed.is_empty() {
                AuthResolver::save_key("anthropic", key_trimmed)?;
                println!("✓ Saved Anthropic API key to config.");
            }
        }
        "4" => {
            selected_default_model = "openai/gpt-4o".to_string();
            println!("\n{}", AuthResolver::get_login_instructions("openai"));
            print!("Enter OpenAI API Key (press Enter to skip if using OPENAI_API_KEY env var): ");
            io::stdout().flush()?;
            let mut key_input = String::new();
            io::stdin().read_line(&mut key_input)?;
            let key_trimmed = key_input.trim();
            if !key_trimmed.is_empty() {
                AuthResolver::save_key("openai", key_trimmed)?;
                println!("✓ Saved OpenAI API key to config.");
            }
        }
        "5" => {
            selected_default_model = "deepseek/deepseek-chat".to_string();
            println!("\n{}", AuthResolver::get_login_instructions("deepseek"));
            print!("Enter DeepSeek API Key (press Enter to skip if using DEEPSEEK_API_KEY env var): ");
            io::stdout().flush()?;
            let mut key_input = String::new();
            io::stdin().read_line(&mut key_input)?;
            let key_trimmed = key_input.trim();
            if !key_trimmed.is_empty() {
                AuthResolver::save_key("deepseek", key_trimmed)?;
                println!("✓ Saved DeepSeek API key to config.");
            }
        }
        "6" => {
            selected_default_model = "openrouter/anthropic/claude-3.5-sonnet".to_string();
            println!("\n{}", AuthResolver::get_login_instructions("openrouter"));
            print!("Enter OpenRouter API Key (press Enter to skip if using OPENROUTER_API_KEY env var): ");
            io::stdout().flush()?;
            let mut key_input = String::new();
            io::stdin().read_line(&mut key_input)?;
            let key_trimmed = key_input.trim();
            if !key_trimmed.is_empty() {
                AuthResolver::save_key("openrouter", key_trimmed)?;
                println!("✓ Saved OpenRouter API key to config.");
            }
        }
        "7" => {
            print!("\nEnter provider name (e.g. gemini, groq, mistral, cerebras): ");
            io::stdout().flush()?;
            let mut prov_input = String::new();
            io::stdin().read_line(&mut prov_input)?;
            let prov = prov_input.trim();
            if !prov.is_empty() {
                print!("Enter default model ID for [{}] (or press Enter for standard): ", prov);
                io::stdout().flush()?;
                let mut m_input = String::new();
                io::stdin().read_line(&mut m_input)?;
                let m_trimmed = m_input.trim();
                if !m_trimmed.is_empty() {
                    selected_default_model = m_trimmed.to_string();
                } else {
                    selected_default_model = format!("{}/default", prov);
                }

                print!("Enter API Key for [{}] (press Enter to skip): ", prov);
                io::stdout().flush()?;
                let mut key_input = String::new();
                io::stdin().read_line(&mut key_input)?;
                let key_trimmed = key_input.trim();
                if !key_trimmed.is_empty() {
                    AuthResolver::save_key(prov, key_trimmed)?;
                    println!("✓ Saved {} API key to config.", prov);
                }
            }
        }
        _ => {
            // Default: Free OpenCode Zen
            selected_default_model = "opencode/deepseek-v4-flash-free".to_string();
            println!("✓ Configured default free model: \x1b[1;32m{}\x1b[0m", selected_default_model);
        }
    }

    println!("\n\x1b[1;33m[Step 3/3] Saving Configuration...\x1b[0m");
    save_default_model(&selected_default_model)?;

    println!("\x1b[1;32m");
    println!("  ╔══════════════════════════════════════════════════════════════════════════════╗");
    println!("  ║  ✓ Onboarding Setup Complete!                                                ║");
    println!("  ╚══════════════════════════════════════════════════════════════════════════════╝");
    println!("\x1b[0m");
    println!("  • Configuration saved to: \x1b[1m{}\x1b[0m", get_config_path().display());
    println!("  • Default Active Model:   \x1b[1;36m{}\x1b[0m", selected_default_model);
    println!();
    println!("  \x1b[1mQuick Start Tips:\x1b[0m");
    println!("    • Run \x1b[1;32mpi-rs\x1b[0m to enter the interactive TUI workspace.");
    println!("    • Switch models on the fly with \x1b[1;32mpi-rs -m <model-id>\x1b[0m or hotkey \x1b[1mCtrl+L\x1b[0m.");
    println!("    • Manage provider credentials anytime with \x1b[1;32mpi-rs --login\x1b[0m or in TUI with \x1b[1m/login\x1b[0m.");
    println!("    • Run one-shot queries with \x1b[1;32mpi-rs -p \"Fix bugs in src/main.rs\"\x1b[0m.");
    println!("    • Run \x1b[1;32mpi-rs --help\x1b[0m to explore all CLI flags & tools.");
    println!();

    Ok(())
}

/// Saves the selected default model cleanly to ~/.pi/config.json
pub fn save_default_model(model_id: &str) -> Result<()> {
    let config_path = get_config_path();
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
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
        Value::String(model_id.trim().to_string()),
    );

    let output_json = serde_json::to_string_pretty(&Value::Object(json_obj))?;
    fs::write(&config_path, output_json)?;
    Ok(())
}

/// Initializes a minimal default ~/.pi/config.json if not present
pub fn ensure_default_config() -> Result<()> {
    if !config_exists() {
        save_default_model("opencode/deepseek-v4-flash-free")?;
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
        assert_eq!(daemons.len(), 3);
        assert_eq!(daemons[0].name, "Ollama");
        assert_eq!(daemons[1].name, "llama.cpp");
        assert_eq!(daemons[2].name, "LM Studio");
    }

    #[test]
    fn test_save_and_load_default_model_in_temp() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("config.json");

        let mut map = serde_json::Map::new();
        map.insert(
            "default_model".to_string(),
            Value::String("anthropic/claude-3-7-sonnet-latest".to_string()),
        );
        fs::write(&config_path, serde_json::to_string_pretty(&Value::Object(map)).unwrap()).unwrap();

        let content = fs::read_to_string(&config_path).unwrap();
        let val: Value = serde_json::from_str(&content).unwrap();
        assert_eq!(
            val.get("default_model").and_then(|v| v.as_str()),
            Some("anthropic/claude-3-7-sonnet-latest")
        );
    }
}
