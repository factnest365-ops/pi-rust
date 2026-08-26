use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AuthCredential {
    #[serde(rename = "api_key")]
    ApiKey { key: String },
    #[serde(rename = "oauth")]
    OAuth {
        access_token: String,
        refresh_token: Option<String>,
        expires_at: Option<i64>,
        token_type: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuthStore {
    #[serde(flatten)]
    pub providers: HashMap<String, AuthCredential>,
}

pub struct AuthResolver;

impl AuthResolver {
    pub fn auth_json_path() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join(".pi").join("agent").join("auth.json")
    }

    pub fn config_json_path() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join(".pi").join("config.json")
    }

    /// Returns all known aliases/synonyms for a provider name
    pub fn provider_aliases(provider: &str) -> Vec<String> {
        let norm = provider.to_lowercase();
        let mut list = vec![norm.clone()];
        match norm.as_str() {
            "anthropic" | "claude" => {
                list.push("anthropic".to_string());
                list.push("claude".to_string());
            }
            "openai" | "gpt" => {
                list.push("openai".to_string());
                list.push("gpt".to_string());
            }
            "gemini" | "google" => {
                list.push("gemini".to_string());
                list.push("google".to_string());
            }
            "opencode" | "zen" | "opencode-go" | "opencode-zen" => {
                list.push("opencode".to_string());
                list.push("zen".to_string());
                list.push("opencode-go".to_string());
                list.push("opencode-zen".to_string());
            }
            "kilo" | "kilo-gateway" => {
                list.push("kilo".to_string());
                list.push("kilo-gateway".to_string());
            }
            "agnes" | "agnes-gateway" => {
                list.push("agnes".to_string());
                list.push("agnes-gateway".to_string());
            }
            "copilot" | "github-copilot" => {
                list.push("copilot".to_string());
                list.push("github-copilot".to_string());
            }
            "bedrock" | "amazon-bedrock" => {
                list.push("bedrock".to_string());
                list.push("amazon-bedrock".to_string());
            }
            "azure" | "azure-openai" => {
                list.push("azure".to_string());
                list.push("azure-openai".to_string());
            }
            "nvidia" | "nvidia-nim" => {
                list.push("nvidia".to_string());
                list.push("nvidia-nim".to_string());
            }
            "together" | "together-ai" => {
                list.push("together".to_string());
                list.push("together-ai".to_string());
            }
            "mistral" | "codestral" => {
                list.push("mistral".to_string());
                list.push("codestral".to_string());
            }
            "moonshot" | "kimi" => {
                list.push("moonshot".to_string());
                list.push("kimi".to_string());
            }
            "xiaomi" | "mimo" => {
                list.push("xiaomi".to_string());
                list.push("mimo".to_string());
            }
            "qwen" | "qwen-token-plan" => {
                list.push("qwen".to_string());
                list.push("qwen-token-plan".to_string());
            }
            "huggingface" | "hf" => {
                list.push("huggingface".to_string());
                list.push("hf".to_string());
            }
            "cloudflare" | "cloudflare-ai-gateway" | "cloudflare-workers-ai" => {
                list.push("cloudflare".to_string());
                list.push("cloudflare-ai-gateway".to_string());
                list.push("cloudflare-workers-ai".to_string());
            }
            "vercel" | "vercel-ai-gateway" => {
                list.push("vercel".to_string());
                list.push("vercel-ai-gateway".to_string());
            }
            _ => {}
        }
        list.dedup();
        list
    }

    /// Resolves credential for a given provider searching env vars, ~/.pi/config.json, ~/.pi/agent/auth.json, and agent configs
    pub fn resolve_key(provider: &str) -> Option<String> {
        let norm = provider.to_lowercase();
        let aliases = Self::provider_aliases(&norm);

        // 1. Check environment variables
        let env_keys = match norm.as_str() {
            "anthropic" | "claude" => vec![
                "ANTHROPIC_API_KEY",
                "ANTHROPIC_OAUTH_TOKEN",
                "CLAUDE_API_KEY",
            ],
            "openai" | "gpt" => vec!["OPENAI_API_KEY"],
            "ant-ling" => vec!["ANT_LING_API_KEY"],
            "azure" | "azure-openai" => vec!["AZURE_OPENAI_API_KEY"],
            "deepseek" => vec!["DEEPSEEK_API_KEY"],
            "nvidia" | "nvidia-nim" => vec!["NVIDIA_API_KEY"],
            "gemini" | "google" => vec!["GEMINI_API_KEY", "GOOGLE_API_KEY", "GOOGLE_AI_KEY"],
            "vertex" | "vertex-ai" => {
                vec!["GOOGLE_CLOUD_API_KEY", "GOOGLE_APPLICATION_CREDENTIALS"]
            }
            "mistral" | "codestral" => vec!["MISTRAL_API_KEY", "CODESTRAL_API_KEY"],
            "groq" => vec!["GROQ_API_KEY"],
            "cerebras" => vec!["CEREBRAS_API_KEY"],
            "cloudflare" | "cloudflare-ai-gateway" | "cloudflare-workers-ai" => {
                vec!["CLOUDFLARE_API_KEY"]
            }
            "xai" => vec!["XAI_API_KEY"],
            "openrouter" => vec!["OPENROUTER_API_KEY"],
            "vercel" | "vercel-ai-gateway" => vec!["AI_GATEWAY_API_KEY"],
            "zai" => vec!["ZAI_API_KEY", "ZAI_CODING_CN_API_KEY"],
            "minimax" => vec!["MINIMAX_API_KEY", "MINIMAX_CN_API_KEY"],
            "together" | "together-ai" => vec!["TOGETHER_API_KEY"],
            "baseten" => vec!["BASETEN_API_KEY"],
            "huggingface" | "hf" => vec!["HF_TOKEN", "HUGGINGFACE_API_KEY"],
            "moonshot" | "kimi" => vec!["MOONSHOT_API_KEY", "KIMI_API_KEY"],
            "copilot" | "github-copilot" => vec!["COPILOT_GITHUB_TOKEN", "GITHUB_TOKEN"],
            "bedrock" | "amazon-bedrock" => vec!["AWS_BEARER_TOKEN_BEDROCK", "AWS_ACCESS_KEY_ID"],
            "opencode" | "zen" | "opencode-go" | "opencode-zen" => {
                vec!["OPENCODE_API_KEY", "ZEN_API_KEY"]
            }
            "fireworks" => vec!["FIREWORKS_API_KEY"],
            "qwen" | "qwen-token-plan" => {
                vec!["QWEN_TOKEN_PLAN_API_KEY", "QWEN_TOKEN_PLAN_CN_API_KEY"]
            }
            "xiaomi" | "mimo" => vec![
                "XIAOMI_API_KEY",
                "XIAOMI_TOKEN_PLAN_CN_API_KEY",
                "XIAOMI_TOKEN_PLAN_AMS_API_KEY",
                "XIAOMI_TOKEN_PLAN_SGP_API_KEY",
            ],
            "kilo" | "kilo-gateway" => vec!["KILO_API_KEY"],
            "agnes" | "agnes-gateway" => vec!["AGNES_API_KEY"],
            _ => vec![],
        };

        for ek in env_keys {
            if let Ok(val) = std::env::var(ek)
                && !val.trim().is_empty()
            {
                return Some(val.trim().to_string());
            }
        }

        // Generic <PROVIDER>_API_KEY fallback across aliases
        for alias in &aliases {
            let generic_env = format!("{}_API_KEY", alias.replace(['-', '.'], "_").to_uppercase());
            if let Ok(val) = std::env::var(generic_env)
                && !val.trim().is_empty()
            {
                return Some(val.trim().to_string());
            }
        }

        // 2. Check ~/.pi/config.json
        if let Ok(content) = fs::read_to_string(Self::config_json_path())
            && let Ok(json) = serde_json::from_str::<serde_json::Value>(&content)
        {
            for alias in &aliases {
                let direct_key = match alias.as_str() {
                    "anthropic" | "claude" => json
                        .get("anthropic_api_key")
                        .or_else(|| json.get("claude_api_key")),
                    "openai" | "gpt" => json
                        .get("openai_api_key")
                        .or_else(|| json.get("gpt_api_key")),
                    "gemini" | "google" => json
                        .get("gemini_api_key")
                        .or_else(|| json.get("google_api_key")),
                    "openrouter" => json.get("openrouter_api_key"),
                    "groq" => json.get("groq_api_key"),
                    "deepseek" => json.get("deepseek_api_key"),
                    "mistral" | "codestral" => json
                        .get("mistral_api_key")
                        .or_else(|| json.get("codestral_api_key")),
                    "opencode" | "zen" | "opencode-zen" | "opencode-go" => json
                        .get("opencode_api_key")
                        .or_else(|| json.get("zen_api_key")),
                    "kilo" | "kilo-gateway" => json.get("kilo_api_key"),
                    "agnes" | "agnes-gateway" => json.get("agnes_api_key"),
                    _ => json.get(format!("{}_api_key", alias)),
                };
                if let Some(key) = direct_key.and_then(|k| k.as_str())
                    && !key.trim().is_empty()
                {
                    return Some(key.trim().to_string());
                }
            }
        }

        // 3. Check ~/.pi/agent/auth.json across all aliases
        if let Ok(content) = fs::read_to_string(Self::auth_json_path())
            && let Ok(json) = serde_json::from_str::<serde_json::Value>(&content)
        {
            for alias in &aliases {
                if let Some(entry) = json.get(alias) {
                    if let Some(k) = entry.get("key").and_then(|v| v.as_str())
                        && !k.trim().is_empty()
                    {
                        return Some(k.trim().to_string());
                    }
                    if let Some(tok) = entry.get("access_token").and_then(|v| v.as_str())
                        && !tok.trim().is_empty()
                    {
                        return Some(tok.trim().to_string());
                    }
                }
            }
        }

        // 4. Check Claude Code config store (~/.claude.json)
        if norm == "anthropic" || norm == "claude" {
            let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
            let claude_cfg = home.join(".claude.json");
            if let Ok(content) = fs::read_to_string(claude_cfg)
                && let Ok(json) = serde_json::from_str::<serde_json::Value>(&content)
                && let Some(k) = json.get("apiKey").and_then(|v| v.as_str())
            {
                return Some(k.trim().to_string());
            }
        }

        None
    }

    /// Stores an API key for a provider in both ~/.pi/config.json and ~/.pi/agent/auth.json
    pub fn save_key(provider: &str, key: &str) -> Result<()> {
        let norm = provider.to_lowercase();
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let pi_dir = home.join(".pi");
        let agent_dir = pi_dir.join("agent");
        let _ = fs::create_dir_all(&agent_dir);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = fs::metadata(&pi_dir) {
                let mut perms = meta.permissions();
                perms.set_mode(0o700);
                let _ = fs::set_permissions(&pi_dir, perms);
            }
            if let Ok(meta) = fs::metadata(&agent_dir) {
                let mut perms = meta.permissions();
                perms.set_mode(0o700);
                let _ = fs::set_permissions(&agent_dir, perms);
            }
        }

        // 1. Update ~/.pi/config.json
        let config_path = pi_dir.join("config.json");
        let mut config_json = if let Ok(c) = fs::read_to_string(&config_path) {
            serde_json::from_str::<serde_json::Value>(&c)
                .ok()
                .filter(|v| v.is_object())
                .unwrap_or_else(|| serde_json::json!({}))
        } else {
            serde_json::json!({})
        };

        let field = match norm.as_str() {
            "anthropic" | "claude" => "anthropic_api_key".to_string(),
            "openai" | "gpt" => "openai_api_key".to_string(),
            "gemini" | "google" => "gemini_api_key".to_string(),
            "openrouter" => "openrouter_api_key".to_string(),
            "groq" => "groq_api_key".to_string(),
            "deepseek" => "deepseek_api_key".to_string(),
            "mistral" | "codestral" => "mistral_api_key".to_string(),
            "opencode" | "zen" | "opencode-zen" | "opencode-go" => "opencode_api_key".to_string(),
            "kilo" | "kilo-gateway" => "kilo_api_key".to_string(),
            "agnes" | "agnes-gateway" => "agnes_api_key".to_string(),
            "cerebras" => "cerebras_api_key".to_string(),
            "xai" => "xai_api_key".to_string(),
            "together" | "together-ai" => "together_api_key".to_string(),
            "fireworks" => "fireworks_api_key".to_string(),
            "perplexity" => "perplexity_api_key".to_string(),
            "copilot" | "github-copilot" => "copilot_api_key".to_string(),
            "qwen" | "qwen-token-plan" => "qwen_api_key".to_string(),
            "xiaomi" | "mimo" => "xiaomi_api_key".to_string(),
            "moonshot" | "kimi" => "moonshot_api_key".to_string(),
            "huggingface" | "hf" => "huggingface_api_key".to_string(),
            "azure" | "azure-openai" => "azure_api_key".to_string(),
            "nvidia" | "nvidia-nim" => "nvidia_api_key".to_string(),
            "bedrock" | "amazon-bedrock" => "bedrock_api_key".to_string(),
            "cloudflare" | "cloudflare-ai-gateway" | "cloudflare-workers-ai" => {
                "cloudflare_api_key".to_string()
            }
            _ => format!("{}_api_key", norm),
        };
        if let Some(obj) = config_json.as_object_mut() {
            obj.insert(field, serde_json::Value::String(key.trim().to_string()));
        }
        fs::write(&config_path, serde_json::to_string_pretty(&config_json)?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = fs::metadata(&config_path) {
                let mut perms = meta.permissions();
                perms.set_mode(0o600);
                let _ = fs::set_permissions(&config_path, perms);
            }
        }

        // 2. Update ~/.pi/agent/auth.json
        let auth_path = agent_dir.join("auth.json");
        let mut auth_json = if let Ok(c) = fs::read_to_string(&auth_path) {
            serde_json::from_str::<serde_json::Value>(&c)
                .ok()
                .filter(|v| v.is_object())
                .unwrap_or_else(|| serde_json::json!({}))
        } else {
            serde_json::json!({})
        };

        if let Some(obj) = auth_json.as_object_mut() {
            obj.insert(
                norm,
                serde_json::json!({
                    "type": "api_key",
                    "key": key.trim()
                }),
            );
        }
        fs::write(&auth_path, serde_json::to_string_pretty(&auth_json)?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = fs::metadata(&auth_path) {
                let mut perms = meta.permissions();
                perms.set_mode(0o600);
                let _ = fs::set_permissions(&auth_path, perms);
            }
        }

        Ok(())
    }

    /// Tests connectivity or credential validity for a given provider with an API key
    pub async fn test_provider_key(provider: &str, key: &str) -> Result<bool> {
        let norm = provider.to_lowercase();
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(4))
            .build()
            .unwrap_or_default();

        let trimmed_key = key.trim();
        if trimmed_key.is_empty() {
            return Ok(false);
        }

        let res = match norm.as_str() {
            "anthropic" | "claude" => {
                client
                    .get("https://api.anthropic.com/v1/models")
                    .header("x-api-key", trimmed_key)
                    .header("anthropic-version", "2023-06-01")
                    .send()
                    .await
            }
            "openai" | "gpt" => {
                client
                    .get("https://api.openai.com/v1/models")
                    .bearer_auth(trimmed_key)
                    .send()
                    .await
            }
            "gemini" | "google" => {
                client
                    .get(format!(
                        "https://generativelanguage.googleapis.com/v1beta/models?key={}",
                        trimmed_key
                    ))
                    .send()
                    .await
            }
            "deepseek" => {
                client
                    .get("https://api.deepseek.com/v1/models")
                    .bearer_auth(trimmed_key)
                    .send()
                    .await
            }
            "groq" => {
                client
                    .get("https://api.groq.com/openai/v1/models")
                    .bearer_auth(trimmed_key)
                    .send()
                    .await
            }
            "openrouter" => {
                client
                    .get("https://openrouter.ai/api/v1/auth/key")
                    .bearer_auth(trimmed_key)
                    .send()
                    .await
            }
            "mistral" | "codestral" => {
                client
                    .get("https://api.mistral.ai/v1/models")
                    .bearer_auth(trimmed_key)
                    .send()
                    .await
            }
            "cerebras" => {
                client
                    .get("https://api.cerebras.ai/v1/models")
                    .bearer_auth(trimmed_key)
                    .send()
                    .await
            }
            _ => {
                return Ok(true);
            }
        };

        match res {
            Ok(r) => Ok(r.status().is_success()),
            Err(e) => Err(anyhow::anyhow!("Connection error: {}", e)),
        }
    }

    /// Initiates a browser-based or interactive login flow for a provider
    pub fn get_login_instructions(provider: &str) -> String {
        match provider.to_lowercase().as_str() {
            "anthropic" | "claude" => {
                "Anthropic Claude Login:\n  1. Obtain an API key from https://console.anthropic.com/settings/keys\n  2. Or run `/login anthropic <key>` / `/auth <key>` to persist credentials.".to_string()
            }
            "openai" | "gpt" => {
                "OpenAI Login:\n  1. Obtain an API key from https://platform.openai.com/api-keys\n  2. Or run `/login openai <key>` / `/auth <key>` to persist credentials.".to_string()
            }
            "gemini" | "google" => {
                "Google Gemini Login:\n  1. Obtain an API key from https://aistudio.google.com/app/apikey\n  2. Or run `/login gemini <key>` / `/auth <key>` to persist credentials.".to_string()
            }
            "openrouter" => {
                "OpenRouter Login:\n  1. Obtain an API key from https://openrouter.ai/keys\n  2. Or run `/login openrouter <key>` to access 200+ models.".to_string()
            }
            "deepseek" => {
                "DeepSeek Login:\n  1. Obtain an API key from https://platform.deepseek.com/api_keys\n  2. Or run `/login deepseek <key>`.".to_string()
            }
            "groq" => {
                "Groq Login:\n  1. Obtain an API key from https://console.groq.com/keys\n  2. Or run `/login groq <key>`.".to_string()
            }
            "cerebras" => {
                "Cerebras Login:\n  1. Obtain an API key from https://cloud.cerebras.ai\n  2. Or run `/login cerebras <key>`.".to_string()
            }
            "mistral" | "codestral" => {
                "Mistral AI Login:\n  1. Obtain an API key from https://console.mistral.ai\n  2. Or run `/login mistral <key>`.".to_string()
            }
            "kilo" | "kilo-gateway" => {
                "Kilo Gateway Login:\n  1. Obtain a key from https://kilo.ai\n  2. Or run `/login kilo <key>`.".to_string()
            }
            "opencode" | "zen" => {
                "OpenCode Zen Login:\n  1. Obtain a key from https://opencode.ai\n  2. Or run `/login opencode <key>`.".to_string()
            }
            "ollama" | "lmstudio" | "llamacpp" | "vllm" => {
                "Local AI Daemon:\n  Zero credentials required. Make sure your local daemon is running on its default port.".to_string()
            }
            _ => format!("Provider Login for [{}]:\n  Run `/login {} <key>` to set your API credentials.", provider, provider),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_resolver_instructions() {
        let claude_info = AuthResolver::get_login_instructions("anthropic");
        assert!(claude_info.contains("console.anthropic.com"));

        let openai_info = AuthResolver::get_login_instructions("openai");
        assert!(openai_info.contains("platform.openai.com"));

        let gemini_info = AuthResolver::get_login_instructions("gemini");
        assert!(gemini_info.contains("aistudio.google.com"));

        let groq_info = AuthResolver::get_login_instructions("groq");
        assert!(groq_info.contains("console.groq.com"));

        let cerebras_info = AuthResolver::get_login_instructions("cerebras");
        assert!(cerebras_info.contains("cloud.cerebras.ai"));

        let mistral_info = AuthResolver::get_login_instructions("mistral");
        assert!(mistral_info.contains("console.mistral.ai"));

        let ollama_info = AuthResolver::get_login_instructions("ollama");
        assert!(ollama_info.contains("Zero credentials required"));

        let vllm_info = AuthResolver::get_login_instructions("vllm");
        assert!(vllm_info.contains("Zero credentials required"));
    }

    #[test]
    fn test_provider_aliases() {
        let claude_aliases = AuthResolver::provider_aliases("claude");
        assert!(claude_aliases.contains(&"anthropic".to_string()));
        assert!(claude_aliases.contains(&"claude".to_string()));

        let gpt_aliases = AuthResolver::provider_aliases("gpt");
        assert!(gpt_aliases.contains(&"openai".to_string()));
        assert!(gpt_aliases.contains(&"gpt".to_string()));

        let google_aliases = AuthResolver::provider_aliases("google");
        assert!(google_aliases.contains(&"gemini".to_string()));
        assert!(google_aliases.contains(&"google".to_string()));

        let custom_aliases = AuthResolver::provider_aliases("my-custom-provider");
        assert_eq!(custom_aliases, vec!["my-custom-provider".to_string()]);
    }

    #[test]
    fn test_generic_env_key_fallback() {
        unsafe {
            std::env::set_var("CUSTOM_AI_API_KEY", "test_key_12345");
        }
        let key = AuthResolver::resolve_key("custom-ai");
        assert_eq!(key.as_deref(), Some("test_key_12345"));
        unsafe {
            std::env::remove_var("CUSTOM_AI_API_KEY");
        }
    }
}
