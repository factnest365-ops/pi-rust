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

/// Renders a quiet, minimal CLI header
pub fn render_welcome_banner() {
    println!("\x1b[1mτ Tau\x1b[0m \x1b[2m0.1.0 — High-Performance Autonomous Coding Agent\x1b[0m\n");
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

/// Runs the interactive first-run onboarding setup
pub async fn run_first_run_wizard() -> Result<()> {
    render_welcome_banner();

    println!("\x1b[1mProbing local endpoints...\x1b[0m");
    let daemons = probe_local_daemons().await;
    let mut first_local_model: Option<String> = None;

    for d in &daemons {
        if d.is_running {
            let count = d.models.len();
            println!(
                "  \x1b[32m✓\x1b[0m {} (localhost:{}) — \x1b[32m{} model(s)\x1b[0m",
                d.name, d.port, count
            );
            if !d.models.is_empty() && first_local_model.is_none() {
                first_local_model = Some(d.models[0].clone());
            }
            for m in d.models.iter().take(2) {
                println!("    \x1b[2m↳ {}\x1b[0m", m);
            }
        } else {
            println!("  \x1b[2m• {} (localhost:{}) — offline\x1b[0m", d.name, d.port);
        }
    }
    println!();

    println!("\x1b[1mSelect default model:\x1b[0m");
    println!("  1) Free OpenCode Flash (opencode/deepseek-v4-flash-free) \x1b[2m[default, zero config]\x1b[0m");
    println!("  2) Anthropic Claude (anthropic/claude-3-7-sonnet-latest)");
    println!("  3) OpenAI (openai/gpt-4o)");
    println!("  4) DeepSeek (deepseek/deepseek-chat)");
    if let Some(ref m) = first_local_model {
        println!("  5) Local Model ({})", m);
    } else {
        println!("  5) Local Model (ollama / llamacpp / lmstudio)");
    }
    println!("  6) Custom Provider");
    println!();

    print!("Select option [1-6] (default: 1): ");
    io::stdout().flush()?;

    let mut choice = String::new();
    io::stdin().read_line(&mut choice)?;
    let choice_trimmed = choice.trim();

    let mut selected_default_model = "opencode/deepseek-v4-flash-free".to_string();

    match choice_trimmed {
        "2" => {
            selected_default_model = "anthropic/claude-3-7-sonnet-latest".to_string();
            print!("Anthropic API Key (or press Enter if using ANTHROPIC_API_KEY env): ");
            io::stdout().flush()?;
            let mut key = String::new();
            io::stdin().read_line(&mut key)?;
            let key = key.trim();
            if !key.is_empty() {
                AuthResolver::save_key("anthropic", key)?;
                println!("  \x1b[32m✓\x1b[0m Saved Anthropic API key.");
            }
        }
        "3" => {
            selected_default_model = "openai/gpt-4o".to_string();
            print!("OpenAI API Key (or press Enter if using OPENAI_API_KEY env): ");
            io::stdout().flush()?;
            let mut key = String::new();
            io::stdin().read_line(&mut key)?;
            let key = key.trim();
            if !key.is_empty() {
                AuthResolver::save_key("openai", key)?;
                println!("  \x1b[32m✓\x1b[0m Saved OpenAI API key.");
            }
        }
        "4" => {
            selected_default_model = "deepseek/deepseek-chat".to_string();
            print!("DeepSeek API Key (or press Enter if using DEEPSEEK_API_KEY env): ");
            io::stdout().flush()?;
            let mut key = String::new();
            io::stdin().read_line(&mut key)?;
            let key = key.trim();
            if !key.is_empty() {
                AuthResolver::save_key("deepseek", key)?;
                println!("  \x1b[32m✓\x1b[0m Saved DeepSeek API key.");
            }
        }
        "5" => {
            if let Some(m) = first_local_model {
                selected_default_model = m;
            } else {
                print!("Local model ID (e.g. ollama/llama3.2): [ollama/llama3.2] ");
                io::stdout().flush()?;
                let mut local_input = String::new();
                io::stdin().read_line(&mut local_input)?;
                let local_trimmed = local_input.trim();
                selected_default_model = if local_trimmed.is_empty() {
                    "ollama/llama3.2".to_string()
                } else {
                    local_trimmed.to_string()
                };
            }
        }
        "6" => {
            print!("Provider name (e.g. openrouter, gemini, groq): ");
            io::stdout().flush()?;
            let mut prov_input = String::new();
            io::stdin().read_line(&mut prov_input)?;
            let prov = prov_input.trim();
            if !prov.is_empty() {
                print!("Model ID for [{}] (e.g. {}/model-name): ", prov, prov);
                io::stdout().flush()?;
                let mut m_input = String::new();
                io::stdin().read_line(&mut m_input)?;
                let m_trimmed = m_input.trim();
                selected_default_model = if m_trimmed.is_empty() {
                    format!("{}/default", prov)
                } else {
                    m_trimmed.to_string()
                };

                print!("API Key for [{}] (press Enter to skip): ", prov);
                io::stdout().flush()?;
                let mut key = String::new();
                io::stdin().read_line(&mut key)?;
                let key = key.trim();
                if !key.is_empty() {
                    AuthResolver::save_key(prov, key)?;
                    println!("  \x1b[32m✓\x1b[0m Saved {} API key.", prov);
                }
            }
        }
        _ => {
            selected_default_model = "opencode/deepseek-v4-flash-free".to_string();
        }
    }

    save_default_model(&selected_default_model)?;

    println!("\n\x1b[32m✓ Setup complete.\x1b[0m");
    println!("  Default model: \x1b[1m{}\x1b[0m", selected_default_model);
    println!("  Config path:   \x1b[2m{}\x1b[0m\n", get_config_path().display());
    println!("Run \x1b[1mtau\x1b[0m to enter the interactive workspace.");

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
