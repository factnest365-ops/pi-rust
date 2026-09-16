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

/// Single-row auth metadata for one provider family.
/// Collapses 6 former match sites (aliases, env_keys, config field,
/// save field, test URL, login instructions) into one lookup.
#[derive(Debug, Clone, Copy)]
pub struct AuthDef {
    pub canonical: &'static str,
    pub display: &'static str,
    pub aliases: &'static [&'static str],
    pub env_keys: &'static [&'static str],
    pub config_field: &'static str,
    /// GET probe URL for `test_provider_key`. `None` means fail closed (`Ok(false)`).
    pub test_url: Option<&'static str>,
    /// Key-issuing console URL. Empty means generic fallback instructions.
    pub login_url: &'static str,
    /// Local daemons need no credentials.
    pub local: bool,
}

static AUTH_DEFS: &[AuthDef] = &[
    AuthDef {
        canonical: "anthropic",
        display: "Anthropic Claude",
        aliases: &["claude"],
        env_keys: &["ANTHROPIC_API_KEY", "ANTHROPIC_OAUTH_TOKEN", "CLAUDE_API_KEY"],
        config_field: "anthropic_api_key",
        test_url: Some("https://api.anthropic.com/v1/models"),
        login_url: "https://console.anthropic.com/settings/keys",
        local: false,
    },
    AuthDef {
        canonical: "openai",
        display: "OpenAI",
        aliases: &["gpt"],
        env_keys: &["OPENAI_API_KEY"],
        config_field: "openai_api_key",
        test_url: Some("https://api.openai.com/v1/models"),
        login_url: "https://platform.openai.com/api-keys",
        local: false,
    },
    AuthDef {
        canonical: "gemini",
        display: "Google Gemini",
        aliases: &["google"],
        env_keys: &["GEMINI_API_KEY", "GOOGLE_API_KEY", "GOOGLE_AI_KEY"],
        config_field: "gemini_api_key",
        test_url: Some("https://generativelanguage.googleapis.com/v1beta/models"),
        login_url: "https://aistudio.google.com/app/apikey",
        local: false,
    },
    AuthDef {
        canonical: "deepseek",
        display: "DeepSeek",
        aliases: &[],
        env_keys: &["DEEPSEEK_API_KEY"],
        config_field: "deepseek_api_key",
        test_url: Some("https://api.deepseek.com/v1/models"),
        login_url: "https://platform.deepseek.com/api_keys",
        local: false,
    },
    AuthDef {
        canonical: "groq",
        display: "Groq",
        aliases: &[],
        env_keys: &["GROQ_API_KEY"],
        config_field: "groq_api_key",
        test_url: Some("https://api.groq.com/openai/v1/models"),
        login_url: "https://console.groq.com/keys",
        local: false,
    },
    AuthDef {
        canonical: "openrouter",
        display: "OpenRouter",
        aliases: &[],
        env_keys: &["OPENROUTER_API_KEY"],
        config_field: "openrouter_api_key",
        test_url: Some("https://openrouter.ai/api/v1/auth/key"),
        login_url: "https://openrouter.ai/keys",
        local: false,
    },
    AuthDef {
        canonical: "mistral",
        display: "Mistral AI",
        aliases: &["codestral"],
        env_keys: &["MISTRAL_API_KEY", "CODESTRAL_API_KEY"],
        config_field: "mistral_api_key",
        test_url: Some("https://api.mistral.ai/v1/models"),
        login_url: "https://console.mistral.ai",
        local: false,
    },
    AuthDef {
        canonical: "cerebras",
        display: "Cerebras",
        aliases: &[],
        env_keys: &["CEREBRAS_API_KEY"],
        config_field: "cerebras_api_key",
        test_url: Some("https://api.cerebras.ai/v1/models"),
        login_url: "https://cloud.cerebras.ai",
        local: false,
    },
    AuthDef {
        canonical: "opencode",
        display: "OpenCode Zen",
        aliases: &["zen", "opencode-go", "opencode-zen"],
        env_keys: &["OPENCODE_API_KEY", "ZEN_API_KEY"],
        config_field: "opencode_api_key",
        test_url: None,
        login_url: "https://opencode.ai",
        local: false,
    },
    AuthDef {
        canonical: "kilo",
        display: "Kilo Gateway",
        aliases: &["kilo-gateway"],
        env_keys: &["KILO_API_KEY"],
        config_field: "kilo_api_key",
        test_url: None,
        login_url: "https://kilo.ai",
        local: false,
    },
    AuthDef {
        canonical: "agnes",
        display: "Agnes Gateway",
        aliases: &["agnes-gateway"],
        env_keys: &["AGNES_API_KEY"],
        config_field: "agnes_api_key",
        test_url: None,
        login_url: "",
        local: false,
    },
    AuthDef {
        canonical: "copilot",
        display: "GitHub Copilot",
        aliases: &["github-copilot"],
        env_keys: &["COPILOT_GITHUB_TOKEN", "GITHUB_TOKEN"],
        config_field: "copilot_api_key",
        test_url: None,
        login_url: "",
        local: false,
    },
    AuthDef {
        canonical: "bedrock",
        display: "Amazon Bedrock",
        aliases: &["amazon-bedrock"],
        env_keys: &["AWS_BEARER_TOKEN_BEDROCK", "AWS_ACCESS_KEY_ID"],
        config_field: "bedrock_api_key",
        test_url: None,
        login_url: "",
        local: false,
    },
    AuthDef {
        canonical: "azure",
        display: "Azure OpenAI",
        aliases: &["azure-openai"],
        env_keys: &["AZURE_OPENAI_API_KEY"],
        config_field: "azure_api_key",
        test_url: None,
        login_url: "",
        local: false,
    },
    AuthDef {
        canonical: "nvidia",
        display: "NVIDIA NIM",
        aliases: &["nvidia-nim"],
        env_keys: &["NVIDIA_API_KEY"],
        config_field: "nvidia_api_key",
        test_url: None,
        login_url: "",
        local: false,
    },
    AuthDef {
        canonical: "together",
        display: "Together AI",
        aliases: &["together-ai"],
        env_keys: &["TOGETHER_API_KEY"],
        config_field: "together_api_key",
        test_url: None,
        login_url: "",
        local: false,
    },
    AuthDef {
        canonical: "moonshot",
        display: "Moonshot Kimi",
        aliases: &["kimi"],
        env_keys: &["MOONSHOT_API_KEY", "KIMI_API_KEY"],
        config_field: "moonshot_api_key",
        test_url: None,
        login_url: "",
        local: false,
    },
    AuthDef {
        canonical: "xiaomi",
        display: "Xiaomi MiMo",
        aliases: &["mimo"],
        env_keys: &[
            "XIAOMI_API_KEY",
            "XIAOMI_TOKEN_PLAN_CN_API_KEY",
            "XIAOMI_TOKEN_PLAN_AMS_API_KEY",
            "XIAOMI_TOKEN_PLAN_SGP_API_KEY",
        ],
        config_field: "xiaomi_api_key",
        test_url: None,
        login_url: "",
        local: false,
    },
    AuthDef {
        canonical: "qwen",
        display: "Qwen",
        aliases: &["qwen-token-plan"],
        env_keys: &["QWEN_TOKEN_PLAN_API_KEY", "QWEN_TOKEN_PLAN_CN_API_KEY"],
        config_field: "qwen_api_key",
        test_url: None,
        login_url: "",
        local: false,
    },
    AuthDef {
        canonical: "huggingface",
        display: "HuggingFace",
        aliases: &["hf"],
        env_keys: &["HF_TOKEN", "HUGGINGFACE_API_KEY"],
        config_field: "huggingface_api_key",
        test_url: None,
        login_url: "",
        local: false,
    },
    AuthDef {
        canonical: "cloudflare",
        display: "Cloudflare",
        aliases: &["cloudflare-ai-gateway", "cloudflare-workers-ai"],
        env_keys: &["CLOUDFLARE_API_KEY"],
        config_field: "cloudflare_api_key",
        test_url: None,
        login_url: "",
        local: false,
    },
    AuthDef {
        canonical: "vercel",
        display: "Vercel AI Gateway",
        aliases: &["vercel-ai-gateway"],
        env_keys: &["AI_GATEWAY_API_KEY"],
        config_field: "vercel_api_key",
        test_url: None,
        login_url: "",
        local: false,
    },
    AuthDef {
        canonical: "xai",
        display: "xAI",
        aliases: &[],
        env_keys: &["XAI_API_KEY"],
        config_field: "xai_api_key",
        test_url: None,
        login_url: "",
        local: false,
    },
    AuthDef {
        canonical: "zai",
        display: "Z.AI",
        aliases: &[],
        env_keys: &["ZAI_API_KEY", "ZAI_CODING_CN_API_KEY"],
        config_field: "zai_api_key",
        test_url: None,
        login_url: "",
        local: false,
    },
    AuthDef {
        canonical: "minimax",
        display: "MiniMax",
        aliases: &[],
        env_keys: &["MINIMAX_API_KEY", "MINIMAX_CN_API_KEY"],
        config_field: "minimax_api_key",
        test_url: None,
        login_url: "",
        local: false,
    },
    AuthDef {
        canonical: "fireworks",
        display: "Fireworks AI",
        aliases: &[],
        env_keys: &["FIREWORKS_API_KEY"],
        config_field: "fireworks_api_key",
        test_url: None,
        login_url: "",
        local: false,
    },
    AuthDef {
        canonical: "perplexity",
        display: "Perplexity",
        aliases: &[],
        env_keys: &["PERPLEXITY_API_KEY"],
        config_field: "perplexity_api_key",
        test_url: None,
        login_url: "",
        local: false,
    },
    AuthDef {
        canonical: "baseten",
        display: "Baseten",
        aliases: &[],
        env_keys: &["BASETEN_API_KEY"],
        config_field: "baseten_api_key",
        test_url: None,
        login_url: "",
        local: false,
    },
    AuthDef {
        canonical: "ant-ling",
        display: "Ant Ling",
        aliases: &[],
        env_keys: &["ANT_LING_API_KEY"],
        config_field: "ant_ling_api_key",
        test_url: None,
        login_url: "",
        local: false,
    },
    AuthDef {
        canonical: "vertex",
        display: "Vertex AI",
        aliases: &["vertex-ai"],
        env_keys: &["GOOGLE_CLOUD_API_KEY", "GOOGLE_APPLICATION_CREDENTIALS"],
        config_field: "vertex_api_key",
        test_url: None,
        login_url: "",
        local: false,
    },
    AuthDef {
        canonical: "ollama",
        display: "Ollama",
        aliases: &[],
        env_keys: &[],
        config_field: "ollama_api_key",
        test_url: None,
        login_url: "",
        local: true,
    },
    AuthDef {
        canonical: "lmstudio",
        display: "LM Studio",
        aliases: &[],
        env_keys: &[],
        config_field: "lmstudio_api_key",
        test_url: None,
        login_url: "",
        local: true,
    },
    AuthDef {
        canonical: "llamacpp",
        display: "llama.cpp",
        aliases: &[],
        env_keys: &[],
        config_field: "llamacpp_api_key",
        test_url: None,
        login_url: "",
        local: true,
    },
    AuthDef {
        canonical: "vllm",
        display: "vLLM",
        aliases: &[],
        env_keys: &[],
        config_field: "vllm_api_key",
        test_url: None,
        login_url: "",
        local: true,
    },
];

static UNKNOWN_DEF: AuthDef = AuthDef {
    canonical: "unknown",
    display: "Unknown",
    aliases: &[],
    env_keys: &[],
    config_field: "",
    test_url: None,
    login_url: "",
    local: false,
};

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

    /// Single lookup for all auth metadata. Unknown providers return
    /// `UNKNOWN_DEF` (fail-closed: no test URL, generic field/instructions).
    pub fn auth_def(provider: &str) -> &'static AuthDef {
        let norm = provider.to_lowercase();
        let n = norm.as_str();
        for def in AUTH_DEFS {
            if n == def.canonical || def.aliases.contains(&n) {
                return def;
            }
        }
        &UNKNOWN_DEF
    }

    /// Returns all known aliases/synonyms for a provider name, driven by [`AuthDef::aliases`].
    pub fn provider_aliases(provider: &str) -> Vec<String> {
        let norm = provider.to_lowercase();
        let def = Self::auth_def(&norm);
        if def.canonical == UNKNOWN_DEF.canonical {
            return vec![norm];
        }
        let mut list = vec![norm.clone()];
        list.push(def.canonical.to_string());
        for a in def.aliases {
            list.push(a.to_string());
        }
        list.dedup();
        list
    }

    /// Resolves credential for a given provider searching env vars, ~/.pi/config.json, ~/.pi/agent/auth.json, and agent configs
    pub fn resolve_key(provider: &str) -> Option<String> {
        let norm = provider.to_lowercase();
        let def = Self::auth_def(&norm);
        let aliases = Self::provider_aliases(&norm);

        // 1. Check environment variables (table-driven + generic alias fallback)
        for ek in def.env_keys {
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

        // 2. Check ~/.pi/config.json (generic over aliases: <alias>_api_key)
        if let Ok(content) = fs::read_to_string(Self::config_json_path())
            && let Ok(json) = serde_json::from_str::<serde_json::Value>(&content)
        {
            for alias in &aliases {
                let field = format!("{}_api_key", alias.replace('-', "_"));
                if let Some(key) = json.get(&field).and_then(|k| k.as_str())
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
        if def.canonical == "anthropic" {
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
        let def = Self::auth_def(&norm);
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

        let field = if def.config_field.is_empty() {
            format!("{}_api_key", norm)
        } else {
            def.config_field.to_string()
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

    /// Tests connectivity or credential validity for a given provider with an API key.
    /// Fail-closed: providers without a `test_url` return `Ok(false)`.
    /// NOTE: only 8 providers currently have probe URLs; the remaining
    /// table entries (gateways, untested APIs, locals, unknown) report
    /// `Ok(false)` and surface as Auth-needed until URLs are added. Expected.
    pub async fn test_provider_key(provider: &str, key: &str) -> Result<bool> {
        let def = Self::auth_def(provider);
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(4))
            .build()
            .unwrap_or_default();

        let trimmed_key = key.trim();
        if trimmed_key.is_empty() {
            return Ok(false);
        }

        let Some(url) = def.test_url else {
            return Ok(false);
        };

        let res = if def.canonical == "anthropic" {
            client
                .get(url)
                .header("x-api-key", trimmed_key)
                .header("anthropic-version", "2023-06-01")
                .send()
                .await
        } else if def.canonical == "gemini" {
            client
                .get(format!("{}?key={}", url, trimmed_key))
                .send()
                .await
        } else {
            client.get(url).bearer_auth(trimmed_key).send().await
        };

        match res {
            Ok(r) => Ok(r.status().is_success()),
            Err(e) => Err(anyhow::anyhow!("Connection error: {}", e)),
        }
    }

    /// Login instructions rendered from [`AuthDef`] (`display` + `login_url`).
    pub fn get_login_instructions(provider: &str) -> String {
        let def = Self::auth_def(provider);
        if def.local {
            return "Local AI Daemon:\n  Zero credentials required. Make sure your local daemon is running on its default port.".to_string();
        }
        if def.login_url.is_empty() {
            return format!(
                "Provider Login for [{}]:\n  Run `/login {} <key>` to set your API credentials.",
                provider, provider
            );
        }
        format!(
            "{} Login:\n  1. Obtain an API key from {}\n  2. Or run `/login {} <key>` to persist credentials.",
            def.display, def.login_url, def.canonical
        )
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

    #[test]
    fn test_auth_def_single_lookup() {
        // One lookup drives aliases, env keys, config field, test URL, login text.
        let def = AuthResolver::auth_def("claude");
        assert_eq!(def.canonical, "anthropic");
        assert!(def.env_keys.contains(&"ANTHROPIC_API_KEY"));
        assert_eq!(def.config_field, "anthropic_api_key");
        assert!(def.test_url.is_some());
        assert!(def.login_url.contains("console.anthropic.com"));

        let gpt = AuthResolver::auth_def("gpt");
        assert_eq!(gpt.canonical, "openai");
        assert_eq!(gpt.config_field, "openai_api_key");

        let unknown = AuthResolver::auth_def("definitely-unknown-provider-xyz");
        assert_eq!(unknown.canonical, "unknown");
        assert!(unknown.test_url.is_none());
        let instr = AuthResolver::get_login_instructions("definitely-unknown-provider-xyz");
        assert!(instr.contains("/login definitely-unknown-provider-xyz <key>"));
    }

    #[tokio::test]
    async fn test_unknown_provider_fails_closed() {
        // `pi --login unknown` must fail closed: no probe URL -> Ok(false).
        let res = AuthResolver::test_provider_key("definitely-unknown-provider-xyz", "some-key")
            .await
            .unwrap();
        assert!(!res);

        let empty = AuthResolver::test_provider_key("openai", "").await.unwrap();
        assert!(!empty);

        // Untested table entries also fail closed until probe URLs are added.
        let kilo = AuthResolver::test_provider_key("kilo", "some-key").await.unwrap();
        assert!(!kilo);
    }
}
