use anyhow::Result;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

pub mod auth;
pub mod constants;
pub mod models;
pub mod tokens;
pub use auth::{AuthCredential, AuthResolver, AuthStore};
pub use constants::{
    DEFAULT_MODEL, LLAMACPP_DEFAULT_ENDPOINT, LLAMACPP_DEFAULT_HOST, LMSTUDIO_DEFAULT_ENDPOINT,
    LMSTUDIO_DEFAULT_HOST, OLLAMA_API_TAGS, OLLAMA_DEFAULT_ENDPOINT, OLLAMA_DEFAULT_HOST,
    OLLAMA_V1_PATH, VLLM_DEFAULT_ENDPOINT, VLLM_DEFAULT_HOST,
};
pub use models::{ModelCatalogLoader, ModelInfo};
pub use tokens::{ContextBudget, TokenProfiler};

#[derive(Debug, Clone)]
pub struct LocalDaemonInfo {
    pub name: &'static str,
    pub port: u16,
    pub url: String,
    pub is_running: bool,
    pub models: Vec<String>,
}

pub async fn discover_local_providers(client: &reqwest::Client) -> Vec<LocalDaemonInfo> {
    let probe_ollama = async {
        let mut d = LocalDaemonInfo {
            name: "Ollama",
            port: 11434,
            url: format!("{}{}", OLLAMA_DEFAULT_HOST, OLLAMA_API_TAGS),
            is_running: false,
            models: Vec::new(),
        };
        if let Ok(res) = client.get(&d.url).send().await
            && res.status().is_success()
            && let Ok(json) = res.json::<serde_json::Value>().await
            && let Some(arr) = json.get("models").and_then(|m| m.as_array())
        {
            d.is_running = true;
            for item in arr {
                if let Some(name) = item.get("name").and_then(|n| n.as_str()) {
                    d.models.push(format!("ollama/{}", name));
                }
            }
        }
        d
    };

    let probe_llamacpp = async {
        let mut d = LocalDaemonInfo {
            name: "llama.cpp",
            port: 8080,
            url: format!("{}{}{}", LLAMACPP_DEFAULT_HOST, OLLAMA_V1_PATH, "/models"),
            is_running: false,
            models: Vec::new(),
        };
        if let Ok(res) = client.get(&d.url).send().await
            && res.status().is_success()
            && let Ok(json) = res.json::<serde_json::Value>().await
            && let Some(arr) = json.get("data").and_then(|d| d.as_array())
        {
            d.is_running = true;
            for item in arr {
                if let Some(id) = item.get("id").and_then(|n| n.as_str()) {
                    d.models.push(format!("llamacpp/{}", id));
                }
            }
        }
        d
    };

    let probe_lmstudio = async {
        let mut d = LocalDaemonInfo {
            name: "LM Studio",
            port: 1234,
            url: format!("{}{}{}", LMSTUDIO_DEFAULT_HOST, OLLAMA_V1_PATH, "/models"),
            is_running: false,
            models: Vec::new(),
        };
        if let Ok(res) = client.get(&d.url).send().await
            && res.status().is_success()
            && let Ok(json) = res.json::<serde_json::Value>().await
            && let Some(arr) = json.get("data").and_then(|d| d.as_array())
        {
            d.is_running = true;
            for item in arr {
                if let Some(id) = item.get("id").and_then(|n| n.as_str()) {
                    d.models.push(format!("lmstudio/{}", id));
                }
            }
        }
        d
    };

    let probe_vllm = async {
        let mut d = LocalDaemonInfo {
            name: "vLLM",
            port: 8000,
            url: format!("{}{}{}", VLLM_DEFAULT_HOST, OLLAMA_V1_PATH, "/models"),
            is_running: false,
            models: Vec::new(),
        };
        if let Ok(res) = client.get(&d.url).send().await
            && res.status().is_success()
            && let Ok(json) = res.json::<serde_json::Value>().await
            && let Some(arr) = json.get("data").and_then(|d| d.as_array())
        {
            d.is_running = true;
            for item in arr {
                if let Some(id) = item.get("id").and_then(|n| n.as_str()) {
                    d.models.push(format!("vllm/{}", id));
                }
            }
        }
        d
    };

    let (ollama, llamacpp, lmstudio, vllm) =
        tokio::join!(probe_ollama, probe_llamacpp, probe_lmstudio, probe_vllm);
    vec![ollama, llamacpp, lmstudio, vllm]
}

static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn get_http_client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default()
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PiConfig {
    pub opencode_api_key: Option<String>,
    pub opencode_base_url: Option<String>,
    pub kilo_api_key: Option<String>,
    pub kilo_base_url: Option<String>,
    pub agnes_api_key: Option<String>,
    pub agnes_base_url: Option<String>,
    pub anthropic_api_key: Option<String>,
    pub openai_api_key: Option<String>,
    pub gemini_api_key: Option<String>,
    pub openrouter_api_key: Option<String>,
    pub deepseek_api_key: Option<String>,
    pub groq_api_key: Option<String>,
    pub cerebras_api_key: Option<String>,
    pub mistral_api_key: Option<String>,
    pub default_model: Option<String>,
    pub default_mode: Option<String>,
    pub default_specialist: Option<String>,
    pub theme: Option<String>,
    pub alfred_level: Option<String>,
    #[serde(flatten)]
    pub custom_keys: std::collections::BTreeMap<String, serde_json::Value>,
}

impl PiConfig {
    fn config_path() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let pi_dir = home.join(".pi");
        let _ = fs::create_dir_all(&pi_dir);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = fs::metadata(&pi_dir) {
                let mut perms = meta.permissions();
                perms.set_mode(0o700);
                let _ = fs::set_permissions(&pi_dir, perms);
            }
        }
        pi_dir.join("config.json")
    }

    fn agent_auth_path() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join(".pi").join("agent").join("auth.json")
    }

    pub fn load() -> Self {
        let mut cfg = if let Ok(content) = fs::read_to_string(Self::config_path()) {
            serde_json::from_str::<PiConfig>(&content).unwrap_or_default()
        } else {
            Self::default()
        };

        // Automatically load existing keys from standard Pi Agent auth store (~/.pi/agent/auth.json)
        if let Ok(json) = fs::read_to_string(Self::agent_auth_path())
            .map_err(anyhow::Error::from)
            .and_then(|c| {
                serde_json::from_str::<serde_json::Value>(&c).map_err(anyhow::Error::from)
            })
            && let Some(obj) = json.as_object()
        {
            for (prov, val) in obj {
                let key = val
                    .get("key")
                    .and_then(|k| k.as_str())
                    .map(ToString::to_string);
                if let Some(k) = key {
                    match prov.as_str() {
                        "opencode" => {
                            if cfg.opencode_api_key.is_none() {
                                cfg.opencode_api_key = Some(k);
                            }
                        }
                        "kilo" => {
                            if cfg.kilo_api_key.is_none() {
                                cfg.kilo_api_key = Some(k);
                            }
                        }
                        "anthropic" => {
                            if cfg.anthropic_api_key.is_none() {
                                cfg.anthropic_api_key = Some(k);
                            }
                        }
                        "openai" => {
                            if cfg.openai_api_key.is_none() {
                                cfg.openai_api_key = Some(k);
                            }
                        }
                        "gemini" | "google" => {
                            if cfg.gemini_api_key.is_none() {
                                cfg.gemini_api_key = Some(k);
                            }
                        }
                        "openrouter" => {
                            if cfg.openrouter_api_key.is_none() {
                                cfg.openrouter_api_key = Some(k);
                            }
                        }
                        "deepseek" => {
                            if cfg.deepseek_api_key.is_none() {
                                cfg.deepseek_api_key = Some(k);
                            }
                        }
                        "groq" => {
                            if cfg.groq_api_key.is_none() {
                                cfg.groq_api_key = Some(k);
                            }
                        }
                        "cerebras" => {
                            if cfg.cerebras_api_key.is_none() {
                                cfg.cerebras_api_key = Some(k);
                            }
                        }
                        "mistral" => {
                            if cfg.mistral_api_key.is_none() {
                                cfg.mistral_api_key = Some(k);
                            }
                        }
                        _ => {
                            cfg.custom_keys
                                .entry(format!("{}_api_key", prov))
                                .or_insert(serde_json::Value::String(k));
                        }
                    }
                }
            }
        }

        cfg
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        let json = serde_json::to_string_pretty(self)?;
        fs::write(&path, json)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = fs::metadata(&path) {
                let mut perms = meta.permissions();
                perms.set_mode(0o600);
                let _ = fs::set_permissions(&path, perms);
            }
        }
        Ok(())
    }

    pub fn set_api_key(&mut self, provider: &str, key: String) -> Result<()> {
        let norm = provider.to_lowercase();
        match norm.as_str() {
            "opencode" | "opencode-zen" | "zen" => self.opencode_api_key = Some(key),
            "kilo" | "kilo-gateway" => self.kilo_api_key = Some(key),
            "agnes" | "agnes-gateway" => self.agnes_api_key = Some(key),
            "anthropic" | "claude" => self.anthropic_api_key = Some(key),
            "openai" | "gpt" => self.openai_api_key = Some(key),
            "gemini" | "google" => self.gemini_api_key = Some(key),
            "openrouter" => self.openrouter_api_key = Some(key),
            "deepseek" => self.deepseek_api_key = Some(key),
            "groq" => self.groq_api_key = Some(key),
            "cerebras" => self.cerebras_api_key = Some(key),
            "mistral" => self.mistral_api_key = Some(key),
            _ => {
                self.custom_keys
                    .insert(format!("{}_api_key", norm), serde_json::Value::String(key));
            }
        }
        self.save()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCatalog {
    pub last_refreshed: String,
    pub models: Vec<String>,
}

impl ModelCatalog {
    fn cache_path() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let pi_dir = home.join(".pi");
        let _ = fs::create_dir_all(&pi_dir);
        pi_dir.join("models_cache.json")
    }

    pub fn default_models() -> Vec<String> {
        vec![
            "anthropic/claude-3-7-sonnet-latest".to_string(),
            "anthropic/claude-3-5-sonnet-latest".to_string(),
            "openai/gpt-4o".to_string(),
            "openai/o3-mini".to_string(),
            "gemini/gemini-2.0-flash".to_string(),
            "deepseek/deepseek-reasoner".to_string(),
            "deepseek/deepseek-chat".to_string(),
            "groq/llama-3.3-70b-versatile".to_string(),
        ]
    }

    pub async fn get_models(force_refresh: bool) -> Vec<String> {
        let path = Self::cache_path();

        if !force_refresh {
            let cached_models = fs::read_to_string(&path)
                .ok()
                .and_then(|c| serde_json::from_str::<ModelCatalog>(&c).ok())
                .and_then(|catalog| {
                    let ts = chrono::DateTime::parse_from_rfc3339(&catalog.last_refreshed).ok()?;
                    let age = chrono::Utc::now().signed_duration_since(ts);
                    if age.num_hours() < 24 && !catalog.models.is_empty() {
                        Some(catalog.models)
                    } else {
                        None
                    }
                });

            if let Some(models) = cached_models {
                return models;
            }
        }

        // Refresh models from default gateways and local daemons
        let mut list = Self::default_models();
        let client = get_http_client();
        let daemons = discover_local_providers(client).await;
        for daemon in daemons {
            if daemon.is_running {
                list.extend(daemon.models);
            }
        }

        let catalog = ModelCatalog {
            last_refreshed: chrono::Utc::now().to_rfc3339(),
            models: list.clone(),
        };

        if let Ok(json) = serde_json::to_string_pretty(&catalog) {
            let _ = fs::write(path, json);
        }

        list
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelConfig {
    pub provider: String,
    pub model_id: String,
    pub api_key: String,
    pub base_url: Option<String>,
    #[serde(default = "default_context_window")]
    pub context_window: usize,
    #[serde(default = "default_max_output")]
    pub max_output: usize,
}

fn default_context_window() -> usize {
    128_000
}

fn default_max_output() -> usize {
    8_192
}

impl ModelConfig {
    fn lookup_models_json_base_url(provider: &str) -> Option<String> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let path = home.join(".pi").join("agent").join("models.json");
        let content = fs::read_to_string(path).ok()?;
        let json: serde_json::Value = serde_json::from_str(&content).ok()?;
        json.get("providers")?
            .get(provider)?
            .get("baseUrl")?
            .as_str()
            .map(ToString::to_string)
    }

    pub fn resolve(raw_model: &str) -> Self {
        let (provider, model_id) = if let Some(m) = raw_model.strip_prefix("openrouter/") {
            ("openrouter", m)
        } else if let Some(m) = raw_model.strip_prefix("ollama/") {
            ("ollama", m)
        } else if let Some(m) = raw_model.strip_prefix("llamacpp/") {
            ("llamacpp", m)
        } else if let Some(m) = raw_model.strip_prefix("lmstudio/") {
            ("lmstudio", m)
        } else if let Some(m) = raw_model.strip_prefix("vllm/") {
            ("vllm", m)
        } else if let Some((p, m)) = raw_model.split_once('/') {
            (p, m)
        } else if raw_model == "big-pickle"
            || raw_model.ends_with("-free")
            || raw_model.contains("zen")
            || raw_model.contains("opencode")
        {
            ("opencode", raw_model)
        } else if raw_model.contains("claude") || raw_model.contains("anthropic") {
            ("anthropic", raw_model)
        } else if raw_model.contains("gemini") || raw_model.contains("google") {
            ("gemini", raw_model)
        } else if raw_model.contains("deepseek") {
            ("deepseek", raw_model)
        } else if raw_model.contains("groq") {
            ("groq", raw_model)
        } else if raw_model.contains("openrouter") {
            ("openrouter", raw_model)
        } else if raw_model.contains("openai")
            || raw_model.starts_with("gpt-")
            || raw_model.starts_with("o1")
            || raw_model.starts_with("o3")
        {
            ("openai", raw_model)
        } else if raw_model.contains("agnes") {
            ("agnes", raw_model)
        } else if raw_model.contains("ollama") {
            ("ollama", raw_model)
        } else if raw_model.contains("llamacpp") || raw_model.contains("llama.cpp") {
            ("llamacpp", raw_model)
        } else if raw_model.contains("lmstudio") {
            ("lmstudio", raw_model)
        } else if raw_model.contains("vllm") {
            ("vllm", raw_model)
        } else if raw_model.contains("mistral") || raw_model.contains("codestral") {
            ("mistral", raw_model)
        } else if raw_model.contains("cerebras") {
            ("cerebras", raw_model)
        } else if raw_model.contains("copilot") {
            ("copilot", raw_model)
        } else if raw_model.contains("bedrock") {
            ("bedrock", raw_model)
        } else {
            ("opencode", raw_model)
        };

        let provider_lower = provider.to_lowercase();
        let norm_provider = match provider_lower.as_str() {
            "claude" => "anthropic",
            "gpt" => "openai",
            "google" => "gemini",
            "zen" | "opencode-zen" | "opencode-go" => "opencode",
            "kilo-gateway" => "kilo",
            "agnes-gateway" => "agnes",
            "together-ai" => "together",
            "github-copilot" => "copilot",
            "amazon-bedrock" => "bedrock",
            "azure-openai" => "azure",
            "nvidia-nim" => "nvidia",
            "llama.cpp" => "llamacpp",
            "cloudflare-ai-gateway" | "cloudflare-workers-ai" => "cloudflare",
            "vercel-ai-gateway" => "vercel",
            "kimi" => "moonshot",
            "mimo" => "xiaomi",
            "qwen-token-plan" => "qwen",
            "hf" => "huggingface",
            "codestral" => "mistral",
            p => p,
        };

        let mut resolved_key = AuthResolver::resolve_key(norm_provider).unwrap_or_default();
        if resolved_key.is_empty() && (norm_provider == "opencode" || norm_provider == "openrouter")
        {
            // Free tier models work with empty or public bearer token
            resolved_key = String::new();
        }

        let custom_base_url = std::env::var(format!(
            "{}_BASE_URL",
            norm_provider.replace(['-', '.'], "_").to_uppercase()
        ))
        .ok()
        .or_else(|| Self::lookup_models_json_base_url(norm_provider));

        let (api_key, default_base_url) = match norm_provider {
            "opencode" => (resolved_key, "https://opencode.ai/zen/v1".to_string()),
            "agnes" => (resolved_key, "https://api.agnes.ai/v1".to_string()),
            "kilo" => (resolved_key, "https://api.kilo.ai/api/gateway".to_string()),
            "ollama" => (
                if resolved_key.is_empty() {
                    "ollama".to_string()
                } else {
                    resolved_key
                },
                format!("{}{}", OLLAMA_DEFAULT_HOST, OLLAMA_V1_PATH),
            ),
            "llamacpp" => (
                if resolved_key.is_empty() {
                    "llamacpp".to_string()
                } else {
                    resolved_key
                },
                format!("{}{}", LLAMACPP_DEFAULT_HOST, OLLAMA_V1_PATH),
            ),
            "lmstudio" => (
                if resolved_key.is_empty() {
                    "lmstudio".to_string()
                } else {
                    resolved_key
                },
                format!("{}{}", LMSTUDIO_DEFAULT_HOST, OLLAMA_V1_PATH),
            ),
            "vllm" => (
                if resolved_key.is_empty() {
                    "vllm".to_string()
                } else {
                    resolved_key
                },
                format!("{}{}", VLLM_DEFAULT_HOST, OLLAMA_V1_PATH),
            ),
            "anthropic" => (resolved_key, "https://api.anthropic.com/v1".to_string()),
            "openai" => (resolved_key, "https://api.openai.com/v1".to_string()),
            "gemini" => (
                resolved_key,
                "https://generativelanguage.googleapis.com/v1beta/openai".to_string(),
            ),
            "openrouter" => (resolved_key, "https://openrouter.ai/api/v1".to_string()),
            "deepseek" => (resolved_key, "https://api.deepseek.com/v1".to_string()),
            "groq" => (resolved_key, "https://api.groq.com/openai/v1".to_string()),
            "cerebras" => (resolved_key, "https://api.cerebras.ai/v1".to_string()),
            "mistral" => (resolved_key, "https://api.mistral.ai/v1".to_string()),
            "xai" => (resolved_key, "https://api.x.ai/v1".to_string()),
            "together" => (resolved_key, "https://api.together.xyz/v1".to_string()),
            "fireworks" => (
                resolved_key,
                "https://api.fireworks.ai/inference/v1".to_string(),
            ),
            "perplexity" => (resolved_key, "https://api.perplexity.ai".to_string()),
            "copilot" => (resolved_key, "https://api.githubcopilot.com".to_string()),
            "qwen" => (
                resolved_key,
                "https://dashscope-intl.aliyuncs.com/compatible-mode/v1".to_string(),
            ),
            "xiaomi" => (resolved_key, "https://api.mimo.xiaomi.com/v1".to_string()),
            "moonshot" => (resolved_key, "https://api.moonshot.cn/v1".to_string()),
            "huggingface" => (
                resolved_key,
                "https://api-inference.huggingface.co/v1".to_string(),
            ),
            "azure" => (
                resolved_key,
                "https://models.inference.ai.azure.com".to_string(),
            ),
            "nvidia" => (
                resolved_key,
                "https://integrate.api.nvidia.com/v1".to_string(),
            ),
            "bedrock" => (
                resolved_key,
                "https://bedrock-runtime.us-east-1.amazonaws.com".to_string(),
            ),
            "cloudflare" => (
                resolved_key,
                "https://gateway.ai.cloudflare.com/v1".to_string(),
            ),
            "vercel" => (resolved_key, "https://ai-gateway.vercel.sh/v1".to_string()),
            "zai" => (resolved_key, "https://api.zai.cn/v1".to_string()),
            "minimax" => (resolved_key, "https://api.minimax.chat/v1".to_string()),
            "baseten" => (resolved_key, "https://bridge.baseten.co/v1".to_string()),
            _ => (resolved_key, "https://api.kilo.ai/v1".to_string()),
        };

        let base_url = custom_base_url.or_else(|| Some(default_base_url.to_string()));
        let (context_window, max_output) =
            ModelCatalogLoader::infer_model_limits(raw_model, norm_provider);

        Self {
            provider: norm_provider.to_string(),
            model_id: model_id.to_string(),
            api_key,
            base_url,
            context_window,
            max_output,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<serde_json::Value>,
}

impl ChatMessage {
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
            tool_call_id: None,
            name: None,
            tool_calls: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::new("user", content)
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new("assistant", content)
    }

    pub fn assistant_with_tool_calls(
        content: impl Into<String>,
        tool_calls: serde_json::Value,
    ) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
            tool_call_id: None,
            name: None,
            tool_calls: Some(tool_calls),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self::new("system", content)
    }

    pub fn tool(content: impl Into<String>) -> Self {
        Self::new("tool", content)
    }

    pub fn tool_result(
        tool_call_id: impl Into<String>,
        name: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            role: "tool".to_string(),
            content: content.into(),
            tool_call_id: Some(tool_call_id.into()),
            name: Some(name.into()),
            tool_calls: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderResponse {
    pub text: String,
    pub tool_calls: Vec<ProviderToolCall>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct StreamToolCallState {
    pub id: String,
    pub name: String,
    pub arguments_buf: String,
}

#[derive(Default, Debug, Clone)]
pub struct OpenAiStreamState {
    pub is_thinking: bool,
    pub full_text: String,
    pub tool_calls: BTreeMap<usize, StreamToolCallState>,
}

impl OpenAiStreamState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn process_line<F>(&mut self, line: &str, mut on_chunk: F)
    where
        F: FnMut(String),
    {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(':') {
            return;
        }

        let Some(data_str) = trimmed.strip_prefix("data:") else {
            return;
        };
        let data_str = data_str.trim();
        if data_str.is_empty() || data_str == "[DONE]" {
            return;
        }

        let Ok(json) = serde_json::from_str::<serde_json::Value>(data_str) else {
            return;
        };

        if let Some(choices) = json.get("choices").and_then(|c| c.as_array()) {
            for choice in choices {
                if let Some(delta) = choice.get("delta") {
                    // Extract reasoning tokens (DeepSeek-R1, o1/o3, Qwen-R1)
                    if let Some(reasoning) = delta
                        .get("reasoning_content")
                        .or_else(|| delta.get("reasoning"))
                        .or_else(|| delta.get("thought"))
                        .and_then(|c| c.as_str())
                        && !reasoning.is_empty()
                    {
                        if !self.is_thinking {
                            self.is_thinking = true;
                            self.full_text.push_str("<thinking>\n");
                            on_chunk("<thinking>\n".to_string());
                        }
                        self.full_text.push_str(reasoning);
                        on_chunk(reasoning.to_string());
                    }
                    if let Some(content) = delta.get("content").and_then(|c| c.as_str())
                        && !content.is_empty()
                    {
                        if self.is_thinking {
                            self.is_thinking = false;
                            self.full_text.push_str("\n</thinking>\n\n");
                            on_chunk("\n</thinking>\n\n".to_string());
                        }
                        self.full_text.push_str(content);
                        on_chunk(content.to_string());
                    }
                    if let Some(tool_calls_arr) = delta.get("tool_calls").and_then(|t| t.as_array())
                    {
                        if self.is_thinking {
                            self.is_thinking = false;
                            self.full_text.push_str("\n</thinking>\n\n");
                            on_chunk("\n</thinking>\n\n".to_string());
                        }
                        for tc in tool_calls_arr {
                            let index =
                                tc.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                            let entry = self.tool_calls.entry(index).or_default();
                            if let Some(id) = tc.get("id").and_then(|i| i.as_str())
                                && !id.is_empty()
                            {
                                entry.id = id.to_string();
                            }
                            if let Some(name) = tc
                                .get("function")
                                .and_then(|f| f.get("name"))
                                .and_then(|n| n.as_str())
                                && !name.is_empty()
                                && entry.name.is_empty()
                            {
                                entry.name = name.to_string();
                            }
                            if let Some(args_val) =
                                tc.get("function").and_then(|f| f.get("arguments"))
                            {
                                if let Some(args_str) = args_val.as_str() {
                                    entry.arguments_buf.push_str(args_str);
                                } else if args_val.is_object() || args_val.is_array() {
                                    entry.arguments_buf = args_val.to_string();
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn finish(mut self) -> ProviderResponse {
        if self.is_thinking {
            self.full_text.push_str("\n</thinking>\n\n");
        }
        let tool_calls = self
            .tool_calls
            .into_values()
            .map(|entry| {
                let id = if entry.id.is_empty() {
                    "call_unknown".to_string()
                } else {
                    entry.id
                };
                let parsed_args = if entry.arguments_buf.trim().is_empty() {
                    serde_json::json!({})
                } else {
                    serde_json::from_str(&entry.arguments_buf).unwrap_or(serde_json::json!({}))
                };
                ProviderToolCall {
                    id,
                    name: entry.name,
                    arguments: parsed_args,
                }
            })
            .collect();

        ProviderResponse {
            text: self.full_text,
            tool_calls,
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct AnthropicStreamState {
    pub is_thinking: bool,
    pub current_event: Option<String>,
    pub full_text: String,
    pub tool_calls: BTreeMap<usize, StreamToolCallState>,
}

impl AnthropicStreamState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn process_line<F>(&mut self, line: &str, mut on_chunk: F)
    where
        F: FnMut(String),
    {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(':') {
            return;
        }

        if let Some(event) = trimmed.strip_prefix("event:") {
            self.current_event = Some(event.trim().to_string());
            return;
        }

        let Some(data_str) = trimmed.strip_prefix("data:") else {
            return;
        };
        let data_str = data_str.trim();
        if data_str.is_empty() {
            return;
        }

        let Ok(json) = serde_json::from_str::<serde_json::Value>(data_str) else {
            return;
        };

        let event_type = json
            .get("type")
            .and_then(|t| t.as_str())
            .or(self.current_event.as_deref())
            .unwrap_or("");

        match event_type {
            "content_block_start" => {
                let index = json.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                if let Some(cb) = json.get("content_block") {
                    let cb_type = cb.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    if cb_type == "tool_use" {
                        if self.is_thinking {
                            self.is_thinking = false;
                            self.full_text.push_str("\n</thinking>\n\n");
                            on_chunk("\n</thinking>\n\n".to_string());
                        }
                        let id = cb
                            .get("id")
                            .and_then(|i| i.as_str())
                            .unwrap_or("call_unknown")
                            .to_string();
                        let name = cb
                            .get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("")
                            .to_string();
                        let entry = self.tool_calls.entry(index).or_default();
                        entry.id = id;
                        entry.name = name;
                        if let Some(input_val) = cb.get("input")
                            && input_val.is_object()
                            && matches!(input_val.as_object(), Some(m) if !m.is_empty())
                        {
                            entry.arguments_buf = input_val.to_string();
                        }
                    } else if cb_type == "thinking" {
                        if !self.is_thinking {
                            self.is_thinking = true;
                            self.full_text.push_str("<thinking>\n");
                            on_chunk("<thinking>\n".to_string());
                        }
                        if let Some(th) = cb.get("thinking").and_then(|t| t.as_str())
                            && !th.is_empty()
                        {
                            self.full_text.push_str(th);
                            on_chunk(th.to_string());
                        }
                    }
                }
            }
            "content_block_delta" => {
                let index = json.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                if let Some(delta) = json.get("delta") {
                    let delta_type = delta.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    match delta_type {
                        "thinking_delta" => {
                            if let Some(thinking) = delta.get("thinking").and_then(|t| t.as_str())
                                && !thinking.is_empty()
                            {
                                if !self.is_thinking {
                                    self.is_thinking = true;
                                    self.full_text.push_str("<thinking>\n");
                                    on_chunk("<thinking>\n".to_string());
                                }
                                self.full_text.push_str(thinking);
                                on_chunk(thinking.to_string());
                            }
                        }
                        "text_delta" => {
                            if let Some(text) = delta.get("text").and_then(|t| t.as_str())
                                && !text.is_empty()
                            {
                                if self.is_thinking {
                                    self.is_thinking = false;
                                    self.full_text.push_str("\n</thinking>\n\n");
                                    on_chunk("\n</thinking>\n\n".to_string());
                                }
                                self.full_text.push_str(text);
                                on_chunk(text.to_string());
                            }
                        }
                        "input_json_delta" => {
                            if let Some(partial) =
                                delta.get("partial_json").and_then(|p| p.as_str())
                            {
                                let entry = self.tool_calls.entry(index).or_default();
                                entry.arguments_buf.push_str(partial);
                            }
                        }
                        _ => {}
                    }
                }
            }
            "content_block_stop" if self.is_thinking => {
                self.is_thinking = false;
                self.full_text.push_str("\n</thinking>\n\n");
                on_chunk("\n</thinking>\n\n".to_string());
            }
            _ => {}
        }

        self.current_event = None;
    }

    pub fn finish(mut self) -> ProviderResponse {
        if self.is_thinking {
            self.full_text.push_str("\n</thinking>\n\n");
        }
        let tool_calls = self
            .tool_calls
            .into_values()
            .map(|entry| {
                let id = if entry.id.is_empty() {
                    "call_unknown".to_string()
                } else {
                    entry.id
                };
                let parsed_args = if entry.arguments_buf.trim().is_empty() {
                    serde_json::json!({})
                } else {
                    serde_json::from_str(&entry.arguments_buf).unwrap_or(serde_json::json!({}))
                };
                ProviderToolCall {
                    id,
                    name: entry.name,
                    arguments: parsed_args,
                }
            })
            .collect();

        ProviderResponse {
            text: self.full_text,
            tool_calls,
        }
    }
}

pub struct ProviderClient;

impl ProviderClient {
    /// Formats an array of ChatMessages into Anthropic Messages API format
    /// Enforces alternating user/assistant roles and merges sequential tool_result blocks.
    pub fn format_anthropic_messages(messages: &[ChatMessage]) -> Vec<serde_json::Value> {
        let mut anthropic_messages: Vec<serde_json::Value> = Vec::new();
        for msg in messages {
            if msg.role == "tool" {
                let tool_use_id = msg.tool_call_id.as_deref().unwrap_or("tool_fallback_id");
                let tool_result_block = serde_json::json!({
                    "type": "tool_result",
                    "tool_use_id": tool_use_id,
                    "content": msg.content,
                });
                if let Some(last) = anthropic_messages.last_mut()
                    && last["role"] == "user"
                    && last["content"].is_array()
                {
                    if let Some(arr) = last["content"].as_array_mut() {
                        arr.push(tool_result_block);
                    }
                } else {
                    anthropic_messages.push(serde_json::json!({
                        "role": "user",
                        "content": vec![tool_result_block]
                    }));
                }
            } else if msg.role == "assistant"
                && let Some(ref tc) = msg.tool_calls
                && let Some(tc_arr) = tc.as_array()
            {
                let mut content_blocks = Vec::new();
                if !msg.content.is_empty() {
                    content_blocks.push(serde_json::json!({
                        "type": "text",
                        "text": msg.content
                    }));
                }
                for call in tc_arr {
                    let id = call
                        .get("id")
                        .and_then(|i| i.as_str())
                        .unwrap_or("toolu_unknown");
                    let name = call
                        .get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("");
                    let parsed_args = if let Some(func) = call.get("function") {
                        if let Some(args_val) = func.get("arguments") {
                            if args_val.is_object() {
                                args_val.clone()
                            } else if let Some(a_str) = args_val.as_str() {
                                serde_json::from_str(a_str)
                                    .unwrap_or_else(|_| serde_json::json!({}))
                            } else {
                                serde_json::json!({})
                            }
                        } else {
                            serde_json::json!({})
                        }
                    } else {
                        serde_json::json!({})
                    };
                    content_blocks.push(serde_json::json!({
                        "type": "tool_use",
                        "id": id,
                        "name": name,
                        "input": parsed_args
                    }));
                }
                anthropic_messages.push(serde_json::json!({
                    "role": "assistant",
                    "content": content_blocks
                }));
            } else {
                let role = match msg.role.as_str() {
                    "assistant" => "assistant",
                    _ => "user",
                };
                if let Some(last) = anthropic_messages.last_mut()
                    && last["role"] == role
                {
                    if let Some(prev_text) = last["content"].as_str() {
                        last["content"] =
                            serde_json::json!(format!("{}\n\n{}", prev_text, msg.content));
                    } else if let Some(arr) = last["content"].as_array_mut() {
                        arr.push(serde_json::json!({
                            "type": "text",
                            "text": msg.content
                        }));
                    } else {
                        anthropic_messages.push(serde_json::json!({
                            "role": role,
                            "content": msg.content
                        }));
                    }
                } else {
                    anthropic_messages.push(serde_json::json!({
                        "role": role,
                        "content": msg.content
                    }));
                }
            }
        }

        if anthropic_messages.is_empty() {
            anthropic_messages.push(serde_json::json!({
                "role": "user",
                "content": "Hello"
            }));
        }

        anthropic_messages
    }

    /// Formats tool definitions for Anthropic Messages API
    pub fn format_anthropic_tools(tools: &[serde_json::Value]) -> Vec<serde_json::Value> {
        let mut formatted: Vec<serde_json::Value> = tools
            .iter()
            .map(|t| {
                let name = t.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let description = t.get("description").and_then(|d| d.as_str()).unwrap_or("");
                let input_schema = t
                    .get("parameters")
                    .or_else(|| t.get("input_schema"))
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({"type": "object"}));
                serde_json::json!({
                    "name": name,
                    "description": description,
                    "input_schema": input_schema
                })
            })
            .collect();

        // Attach ephemeral cache_control to the last tool so the entire toolset schema is cached
        if let Some(last_tool) = formatted.last_mut() {
            last_tool["cache_control"] = serde_json::json!({ "type": "ephemeral" });
        }

        formatted
    }

    /// Applies Anthropic prompt caching breakpoints (up to 2 message breakpoints) across conversation history.
    /// Anthropic allows at most 4 breakpoints per request:
    /// - 1 for system prompt (applied in system_blocks)
    /// - 1 for tools definition (applied on the last tool in format_anthropic_tools)
    /// - Up to 2 for conversation history:
    ///     * Message 0 (initial turn / project context)
    ///     * Rolling penultimate turn (len - 2), caching prior turn history so subsequent turns get high cache hit rates
    pub fn apply_anthropic_prompt_caching(messages: &mut [serde_json::Value]) {
        if messages.is_empty() {
            return;
        }

        fn attach_cache_control(msg: &mut serde_json::Value) {
            if let Some(s) = msg.get("content").and_then(|c| c.as_str()) {
                let text = s.to_string();
                msg["content"] = serde_json::json!([
                    {
                        "type": "text",
                        "text": text,
                        "cache_control": { "type": "ephemeral" }
                    }
                ]);
            } else if let Some(arr) = msg.get_mut("content").and_then(|c| c.as_array_mut())
                && let Some(last_block) = arr.last_mut()
            {
                last_block["cache_control"] = serde_json::json!({ "type": "ephemeral" });
            }
        }

        // 1. Always cache the initial message (project context / user instruction)
        attach_cache_control(&mut messages[0]);

        // 2. For multi-turn conversations (>= 3 messages), cache the penultimate turn (completed history prefix)
        if messages.len() >= 3 {
            let penultimate = messages.len() - 2;
            attach_cache_control(&mut messages[penultimate]);
        }
    }

    /// Formats an array of ChatMessages into OpenAI Chat Completions API format
    /// Invariant: role "tool" always transmits "tool_call_id".
    pub fn format_openai_messages(
        system_prompt: &str,
        messages: &[ChatMessage],
    ) -> Vec<serde_json::Value> {
        let mut openai_messages = Vec::new();
        if !system_prompt.is_empty() {
            openai_messages.push(serde_json::json!({
                "role": "system",
                "content": system_prompt
            }));
        }
        for msg in messages {
            let mut msg_obj = serde_json::json!({
                "role": msg.role,
                "content": msg.content
            });
            if let Some(ref tcid) = msg.tool_call_id {
                msg_obj["tool_call_id"] = serde_json::Value::String(tcid.clone());
            } else if msg.role == "tool" {
                msg_obj["tool_call_id"] = serde_json::Value::String("call_unknown".to_string());
            }
            if let Some(ref name) = msg.name {
                msg_obj["name"] = serde_json::Value::String(name.clone());
            }
            if let Some(ref tc) = msg.tool_calls {
                msg_obj["tool_calls"] = tc.clone();
            }
            openai_messages.push(msg_obj);
        }

        if openai_messages.is_empty() {
            openai_messages.push(serde_json::json!({
                "role": "user",
                "content": "Hello"
            }));
        }

        openai_messages
    }

    /// Formats tool definitions for OpenAI Chat Completions API
    pub fn format_openai_tools(tools: &[serde_json::Value]) -> Vec<serde_json::Value> {
        tools
            .iter()
            .map(|t| {
                if t.get("type").and_then(|tp| tp.as_str()) == Some("function") {
                    t.clone()
                } else {
                    serde_json::json!({
                        "type": "function",
                        "function": t
                    })
                }
            })
            .collect()
    }

    pub async fn complete_text(
        config: &ModelConfig,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String> {
        let resp = Self::complete_with_tools(config, system_prompt, user_prompt, &[]).await?;
        Ok(resp.text)
    }

    pub async fn complete_with_tools(
        config: &ModelConfig,
        system_prompt: &str,
        user_prompt: &str,
        tools: &[serde_json::Value],
    ) -> Result<ProviderResponse> {
        Self::stream_with_tools(config, system_prompt, user_prompt, tools, |_| {}).await
    }

    pub async fn stream_with_tools<F>(
        config: &ModelConfig,
        system_prompt: &str,
        user_prompt: &str,
        tools: &[serde_json::Value],
        on_chunk: F,
    ) -> Result<ProviderResponse>
    where
        F: FnMut(String),
    {
        Self::stream_messages_with_tools(
            config,
            system_prompt,
            &[ChatMessage::user(user_prompt)],
            tools,
            on_chunk,
        )
        .await
    }

    pub async fn stream_messages_with_tools<F>(
        config: &ModelConfig,
        system_prompt: &str,
        messages: &[ChatMessage],
        tools: &[serde_json::Value],
        mut on_chunk: F,
    ) -> Result<ProviderResponse>
    where
        F: FnMut(String),
    {
        let client = get_http_client();

        if config.provider == "anthropic" {
            let mut anthropic_messages = Self::format_anthropic_messages(messages);
            Self::apply_anthropic_prompt_caching(&mut anthropic_messages);

            let is_claude_37 =
                config.model_id.contains("3-7") || config.model_id.contains("claude-3-7");
            let max_tokens = if is_claude_37 {
                32000
            } else if config.model_id.contains("3-5") || config.model_id.contains("claude") {
                8192
            } else {
                4096
            };

            // Anthropic Prompt Caching System Blocks
            let system_blocks = serde_json::json!([
                {
                    "type": "text",
                    "text": system_prompt,
                    "cache_control": { "type": "ephemeral" }
                }
            ]);

            let mut body = serde_json::json!({
                "model": config.model_id,
                "max_tokens": max_tokens,
                "system": system_blocks,
                "messages": anthropic_messages,
                "stream": true,
            });

            // Claude 3.7 Hybrid Extended Thinking
            if is_claude_37 {
                body["thinking"] = serde_json::json!({
                    "type": "enabled",
                    "budget_tokens": 16000
                });
            }

            if !tools.is_empty() {
                body["tools"] = serde_json::Value::Array(Self::format_anthropic_tools(tools));
            }

            let base_url = config
                .base_url
                .as_deref()
                .unwrap_or("https://api.anthropic.com/v1");

            let endpoint = if base_url.ends_with("/messages") {
                base_url.to_string()
            } else {
                format!("{}/messages", base_url.trim_end_matches('/'))
            };

            let mut req = client
                .post(&endpoint)
                .header("anthropic-version", "2023-06-01")
                .header("anthropic-beta", "prompt-caching-2024-07-31")
                .header("content-type", "application/json");

            if !config.api_key.is_empty() {
                req = req.header("x-api-key", &config.api_key);
            }

            let res = req.json(&body).send().await?;
            let status = res.status();
            if !status.is_success() {
                let err_text = res.text().await.unwrap_or_default();
                return Err(anyhow::anyhow!(
                    "Anthropic API error ({}): {}",
                    status,
                    err_text
                ));
            }

            let mut stream = res.bytes_stream();
            let mut byte_buf = Vec::new();
            let mut state = AnthropicStreamState::new();

            while let Some(chunk_res) = stream.next().await {
                let chunk = chunk_res?;
                byte_buf.extend_from_slice(&chunk);

                while let Some(pos) = byte_buf.iter().position(|&b| b == b'\n') {
                    let line_bytes = &byte_buf[..pos];
                    let line_str = String::from_utf8_lossy(line_bytes)
                        .trim_end_matches('\r')
                        .to_string();
                    byte_buf.drain(..=pos);
                    state.process_line(&line_str, &mut on_chunk);
                }
            }

            if !byte_buf.is_empty() {
                let line_str = String::from_utf8_lossy(&byte_buf)
                    .trim_end_matches('\r')
                    .to_string();
                if !line_str.is_empty() {
                    state.process_line(&line_str, &mut on_chunk);
                }
            }

            Ok(state.finish())
        } else {
            let base_url = config
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string());

            let endpoint = if base_url.ends_with("/chat/completions") {
                base_url
            } else {
                format!("{}/chat/completions", base_url.trim_end_matches('/'))
            };

            let openai_messages = Self::format_openai_messages(system_prompt, messages);

            let mut body = serde_json::json!({
                "model": config.model_id,
                "messages": openai_messages,
                "stream": true,
            });

            // OpenAI o1 / o3-mini Reasoning Parameters
            let is_o_series = config.model_id.starts_with("o1")
                || config.model_id.starts_with("o3")
                || config.model_id.contains("/o1")
                || config.model_id.contains("/o3");
            if is_o_series {
                body["reasoning_effort"] = serde_json::json!("medium");
            }

            if !tools.is_empty() {
                body["tools"] = serde_json::Value::Array(Self::format_openai_tools(tools));
            }

            let mut req = client.post(&endpoint);
            if !config.api_key.is_empty() {
                req = req.bearer_auth(&config.api_key);
            }
            if config.provider == "openrouter" {
                req = req
                    .header("HTTP-Referer", "https://pi.dev")
                    .header("X-Title", "Tau Coding Agent 2026");
            }

            let res = req.json(&body).send().await?;
            let status = res.status();
            if !status.is_success() {
                let err_text = res.text().await.unwrap_or_default();
                return Err(anyhow::anyhow!(
                    "Provider [{}] API error ({}): {}",
                    config.provider,
                    status,
                    err_text
                ));
            }

            let mut stream = res.bytes_stream();
            let mut byte_buf = Vec::new();
            let mut state = OpenAiStreamState::new();

            while let Some(chunk_res) = stream.next().await {
                let chunk = chunk_res?;
                byte_buf.extend_from_slice(&chunk);

                while let Some(pos) = byte_buf.iter().position(|&b| b == b'\n') {
                    let line_bytes = &byte_buf[..pos];
                    let line_str = String::from_utf8_lossy(line_bytes)
                        .trim_end_matches('\r')
                        .to_string();
                    byte_buf.drain(..=pos);
                    state.process_line(&line_str, &mut on_chunk);
                }
            }

            if !byte_buf.is_empty() {
                let line_str = String::from_utf8_lossy(&byte_buf)
                    .trim_end_matches('\r')
                    .to_string();
                if !line_str.is_empty() {
                    state.process_line(&line_str, &mut on_chunk);
                }
            }

            Ok(state.finish())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_config_resolution() {
        let opencode = ModelConfig::resolve("opencode/zen-coder");
        assert_eq!(opencode.provider, "opencode");
        assert_eq!(opencode.model_id, "zen-coder");
        assert_eq!(
            opencode.base_url.as_deref(),
            Some("https://opencode.ai/zen/v1")
        );

        let kilo = ModelConfig::resolve("kilo/deepseek-r1");
        assert_eq!(kilo.provider, "kilo");
        assert_eq!(kilo.model_id, "deepseek-r1");
        assert_eq!(
            kilo.base_url.as_deref(),
            Some("https://api.kilo.ai/api/gateway")
        );

        let agnes = ModelConfig::resolve("agnes/agnes-core");
        assert_eq!(agnes.provider, "agnes");
        assert_eq!(agnes.model_id, "agnes-core");
        let expected_agnes_base = std::env::var("AGNES_BASE_URL")
            .ok()
            .or_else(|| ModelConfig::lookup_models_json_base_url("agnes"))
            .unwrap_or_else(|| "https://api.agnes.ai/v1".to_string());
        assert_eq!(
            agnes.base_url.as_deref(),
            Some(expected_agnes_base.as_str())
        );

        let claude = ModelConfig::resolve("claude/claude-3-7-sonnet");
        assert_eq!(claude.provider, "anthropic");
        assert_eq!(claude.model_id, "claude-3-7-sonnet");
        let expected_anthropic_base = std::env::var("ANTHROPIC_BASE_URL")
            .unwrap_or_else(|_| "https://api.anthropic.com/v1".to_string());
        assert_eq!(
            claude.base_url.as_deref(),
            Some(expected_anthropic_base.as_str())
        );

        let gemini = ModelConfig::resolve("google/gemini-2.0-flash");
        assert_eq!(gemini.provider, "gemini");
        assert_eq!(gemini.model_id, "gemini-2.0-flash");
        assert_eq!(
            gemini.base_url.as_deref(),
            Some("https://generativelanguage.googleapis.com/v1beta/openai")
        );

        let groq = ModelConfig::resolve("groq/llama-3.3-70b-versatile");
        assert_eq!(groq.provider, "groq");
        assert_eq!(
            groq.base_url.as_deref(),
            Some("https://api.groq.com/openai/v1")
        );

        let cerebras = ModelConfig::resolve("cerebras/llama-3.3-70b");
        assert_eq!(cerebras.provider, "cerebras");
        assert_eq!(
            cerebras.base_url.as_deref(),
            Some("https://api.cerebras.ai/v1")
        );

        let mistral = ModelConfig::resolve("mistral/codestral-latest");
        assert_eq!(mistral.provider, "mistral");
        assert_eq!(
            mistral.base_url.as_deref(),
            Some("https://api.mistral.ai/v1")
        );

        let deepseek = ModelConfig::resolve("deepseek/deepseek-reasoner");
        assert_eq!(deepseek.provider, "deepseek");
        assert_eq!(
            deepseek.base_url.as_deref(),
            Some("https://api.deepseek.com/v1")
        );

        let copilot = ModelConfig::resolve("copilot/gpt-4o");
        assert_eq!(copilot.provider, "copilot");
        assert_eq!(
            copilot.base_url.as_deref(),
            Some("https://api.githubcopilot.com")
        );

        let bedrock = ModelConfig::resolve("bedrock/anthropic.claude-3-5-sonnet");
        assert_eq!(bedrock.provider, "bedrock");
        assert_eq!(
            bedrock.base_url.as_deref(),
            Some("https://bedrock-runtime.us-east-1.amazonaws.com")
        );

        let ollama = ModelConfig::resolve("ollama/llama3");
        assert_eq!(ollama.provider, "ollama");
        assert_eq!(ollama.model_id, "llama3");
        assert_eq!(ollama.api_key, "ollama");
        assert_eq!(
            ollama.base_url.as_deref(),
            Some(format!("{}{}", OLLAMA_DEFAULT_HOST, OLLAMA_V1_PATH).as_str())
        );

        let llamacpp = ModelConfig::resolve("llamacpp/llama-3.2-3b");
        assert_eq!(llamacpp.provider, "llamacpp");
        assert_eq!(llamacpp.model_id, "llama-3.2-3b");
        assert_eq!(llamacpp.api_key, "llamacpp");
        assert_eq!(
            llamacpp.base_url.as_deref(),
            Some(format!("{}{}", LLAMACPP_DEFAULT_HOST, OLLAMA_V1_PATH).as_str())
        );

        let lmstudio = ModelConfig::resolve("lmstudio/mistral-7b");
        assert_eq!(lmstudio.provider, "lmstudio");
        assert_eq!(lmstudio.model_id, "mistral-7b");
        assert_eq!(lmstudio.api_key, "lmstudio");
        assert_eq!(
            lmstudio.base_url.as_deref(),
            Some(format!("{}{}", LMSTUDIO_DEFAULT_HOST, OLLAMA_V1_PATH).as_str())
        );

        let vllm = ModelConfig::resolve("vllm/qwen-2.5");
        assert_eq!(vllm.provider, "vllm");
        assert_eq!(vllm.api_key, "vllm");
        assert_eq!(
            vllm.base_url.as_deref(),
            Some(format!("{}{}", VLLM_DEFAULT_HOST, OLLAMA_V1_PATH).as_str())
        );

        // Fallbacks without slash
        let ollama_fallback = ModelConfig::resolve("local-ollama-llama3");
        assert_eq!(ollama_fallback.provider, "ollama");
        assert_eq!(ollama_fallback.api_key, "ollama");
        assert_eq!(
            ollama_fallback.base_url.as_deref(),
            Some(format!("{}{}", OLLAMA_DEFAULT_HOST, OLLAMA_V1_PATH).as_str())
        );

        let llamacpp_fallback = ModelConfig::resolve("llamacpp-qwen");
        assert_eq!(llamacpp_fallback.provider, "llamacpp");
        assert_eq!(llamacpp_fallback.api_key, "llamacpp");
        assert_eq!(
            llamacpp_fallback.base_url.as_deref(),
            Some(format!("{}{}", LLAMACPP_DEFAULT_HOST, OLLAMA_V1_PATH).as_str())
        );

        let lmstudio_fallback = ModelConfig::resolve("lmstudio-hermes");
        assert_eq!(lmstudio_fallback.provider, "lmstudio");
        assert_eq!(lmstudio_fallback.api_key, "lmstudio");
        assert_eq!(
            lmstudio_fallback.base_url.as_deref(),
            Some(format!("{}{}", LMSTUDIO_DEFAULT_HOST, OLLAMA_V1_PATH).as_str())
        );
    }

    #[test]
    fn test_model_config_env_override() {
        unsafe {
            std::env::set_var("ZAI_BASE_URL", "https://custom-proxy.zai.com/v1");
        }
        let cfg = ModelConfig::resolve("zai/glm-4-flash");
        assert_eq!(
            cfg.base_url.as_deref(),
            Some("https://custom-proxy.zai.com/v1")
        );
        unsafe {
            std::env::remove_var("ZAI_BASE_URL");
        }
    }

    #[test]
    fn test_openai_sse_parsing() {
        let lines = vec![
            ": keepalive\n",
            "data: {\"id\":\"cmpl-1\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello \"}}]}\n",
            "data: {\"id\":\"cmpl-1\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"world!\"}}]}\n",
            "data: {\"id\":\"cmpl-1\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_read_1\",\"type\":\"function\",\"function\":{\"name\":\"read\",\"arguments\":\"{\\\"path\\\": \"}}]}}]}\n",
            "data: {\"id\":\"cmpl-1\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"src/lib.rs\\\"}\"}}]}}]}\n",
            "data: {\"id\":\"cmpl-1\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":1,\"id\":\"call_bash_2\",\"type\":\"function\",\"function\":{\"name\":\"bash\",\"arguments\":\"{\\\"command\\\": \\\"cargo test\\\"}\"}}]}}]}\n",
            "data: [DONE]\n",
        ];

        let mut emitted_chunks = Vec::new();
        let mut state = OpenAiStreamState::new();

        for line in lines {
            state.process_line(line, |chunk| emitted_chunks.push(chunk));
        }

        let resp = state.finish();
        assert_eq!(emitted_chunks, vec!["Hello ", "world!"]);
        assert_eq!(resp.text, "Hello world!");
        assert_eq!(resp.tool_calls.len(), 2);
        assert_eq!(resp.tool_calls[0].id, "call_read_1");
        assert_eq!(resp.tool_calls[0].name, "read");
        assert_eq!(
            resp.tool_calls[0].arguments,
            serde_json::json!({ "path": "src/lib.rs" })
        );
        assert_eq!(resp.tool_calls[1].id, "call_bash_2");
        assert_eq!(resp.tool_calls[1].name, "bash");
        assert_eq!(
            resp.tool_calls[1].arguments,
            serde_json::json!({ "command": "cargo test" })
        );
    }

    #[test]
    fn test_anthropic_sse_parsing() {
        let lines = vec![
            "event: message_start\n",
            "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_123\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude-3-5-sonnet\"}}\n",
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Let me help \"}}\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"with that.\"}}\n",
            "event: content_block_stop\n",
            "data: {\"type\":\"content_block_stop\",\"index\":0}\n",
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_bash_1\",\"name\":\"bash\",\"input\":{}}}\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"command\\\": \"}}\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"\\\"cargo test\\\"}\"}}\n",
            "event: content_block_stop\n",
            "data: {\"type\":\"content_block_stop\",\"index\":1}\n",
            "event: message_delta\n",
            "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"}}\n",
            "event: message_stop\n",
            "data: {\"type\":\"message_stop\"}\n",
        ];

        let mut emitted_chunks = Vec::new();
        let mut state = AnthropicStreamState::new();

        for line in lines {
            state.process_line(line, |chunk| emitted_chunks.push(chunk));
        }

        let resp = state.finish();
        assert_eq!(emitted_chunks, vec!["Let me help ", "with that."]);
        assert_eq!(resp.text, "Let me help with that.");
        assert_eq!(resp.tool_calls.len(), 1);
        assert_eq!(resp.tool_calls[0].id, "toolu_bash_1");
        assert_eq!(resp.tool_calls[0].name, "bash");
        assert_eq!(
            resp.tool_calls[0].arguments,
            serde_json::json!({ "command": "cargo test" })
        );
    }

    #[test]
    fn test_reasoning_and_thinking_sse_parsing() {
        // Test DeepSeek reasoning content
        let openai_lines = vec![
            "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"Thinking step 1... \"}}]}\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"Final answer.\"}}]}\n",
            "data: [DONE]\n",
        ];
        let mut chunks = Vec::new();
        let mut oai_state = OpenAiStreamState::new();
        for line in openai_lines {
            oai_state.process_line(line, |c| chunks.push(c));
        }
        let resp = oai_state.finish();
        assert_eq!(
            chunks,
            vec![
                "<thinking>\n",
                "Thinking step 1... ",
                "\n</thinking>\n\n",
                "Final answer."
            ]
        );
        assert_eq!(
            resp.text,
            "<thinking>\nThinking step 1... \n</thinking>\n\nFinal answer."
        );

        // Test Anthropic thinking delta
        let anthropic_lines = vec![
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"Reasoning about code... \"}}\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Here is the solution.\"}}\n",
        ];
        let mut ant_chunks = Vec::new();
        let mut ant_state = AnthropicStreamState::new();
        for line in anthropic_lines {
            ant_state.process_line(line, |c| ant_chunks.push(c));
        }
        let ant_resp = ant_state.finish();
        assert_eq!(
            ant_chunks,
            vec![
                "<thinking>\n",
                "Reasoning about code... ",
                "\n</thinking>\n\n",
                "Here is the solution."
            ]
        );
        assert_eq!(
            ant_resp.text,
            "<thinking>\nReasoning about code... \n</thinking>\n\nHere is the solution."
        );
    }

    #[test]
    fn test_openai_repeated_tool_name_dedup() {
        let lines = vec![
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"read\",\"arguments\":\"\"}}]}}]}\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"name\":\"read\",\"arguments\":\"{\\\"path\\\":\\\"file.rs\\\"}\"}}]}}]}\n",
            "data: [DONE]\n",
        ];
        let mut state = OpenAiStreamState::new();
        for line in lines {
            state.process_line(line, |_| {});
        }
        let resp = state.finish();
        assert_eq!(resp.tool_calls.len(), 1);
        assert_eq!(resp.tool_calls[0].name, "read");
    }

    #[test]
    fn test_anthropic_message_role_alternation_and_tool_merging() {
        // Multi-turn session: User -> Assistant (calls 2 tools) -> Tool 1 -> Tool 2 -> Assistant
        let messages = vec![
            ChatMessage::user("Search and read file"),
            ChatMessage::assistant_with_tool_calls(
                "Searching now",
                serde_json::json!([
                    {
                        "id": "toolu_search_1",
                        "function": {
                            "name": "find",
                            "arguments": "{\"pattern\": \"*.rs\"}"
                        }
                    },
                    {
                        "id": "toolu_read_2",
                        "function": {
                            "name": "read",
                            "arguments": "{\"path\": \"src/main.rs\"}"
                        }
                    }
                ]),
            ),
            ChatMessage::tool_result("toolu_search_1", "find", "src/main.rs"),
            ChatMessage::tool_result("toolu_read_2", "read", "fn main() {}"),
            ChatMessage::assistant("Found and read main.rs"),
            ChatMessage::user("Please format it"),
            ChatMessage::user("And run cargo check"),
        ];

        let anthropic_msgs = ProviderClient::format_anthropic_messages(&messages);

        // Turn 1: user
        assert_eq!(anthropic_msgs[0]["role"], "user");
        assert_eq!(anthropic_msgs[0]["content"], "Search and read file");

        // Turn 2: assistant with 2 tool_use content blocks
        assert_eq!(anthropic_msgs[1]["role"], "assistant");
        let asst_blocks = anthropic_msgs[1]["content"].as_array().unwrap();
        assert_eq!(asst_blocks.len(), 3); // 1 text + 2 tool_use
        assert_eq!(asst_blocks[0]["type"], "text");
        assert_eq!(asst_blocks[1]["type"], "tool_use");
        assert_eq!(asst_blocks[1]["id"], "toolu_search_1");
        assert_eq!(asst_blocks[2]["type"], "tool_use");
        assert_eq!(asst_blocks[2]["id"], "toolu_read_2");

        // Turn 3: user containing BOTH tool results in a single message
        assert_eq!(anthropic_msgs[2]["role"], "user");
        let tool_result_blocks = anthropic_msgs[2]["content"].as_array().unwrap();
        assert_eq!(tool_result_blocks.len(), 2);
        assert_eq!(tool_result_blocks[0]["type"], "tool_result");
        assert_eq!(tool_result_blocks[0]["tool_use_id"], "toolu_search_1");
        assert_eq!(tool_result_blocks[1]["type"], "tool_result");
        assert_eq!(tool_result_blocks[1]["tool_use_id"], "toolu_read_2");

        // Turn 4: assistant
        assert_eq!(anthropic_msgs[3]["role"], "assistant");

        // Turn 5: user (merged sequential user messages)
        assert_eq!(anthropic_msgs[4]["role"], "user");
        assert_eq!(
            anthropic_msgs[4]["content"],
            "Please format it\n\nAnd run cargo check"
        );

        // Verify strictly alternating roles
        for i in 1..anthropic_msgs.len() {
            assert_ne!(
                anthropic_msgs[i]["role"],
                anthropic_msgs[i - 1]["role"],
                "Anthropic roles must strictly alternate"
            );
        }
    }

    #[test]
    fn test_openai_message_serialization_with_tool_call_id() {
        let messages = vec![
            ChatMessage::user("Run tests"),
            ChatMessage::assistant_with_tool_calls(
                "",
                serde_json::json!([
                    {
                        "id": "call_bash_99",
                        "type": "function",
                        "function": {
                            "name": "bash",
                            "arguments": "{\"command\": \"cargo test\"}"
                        }
                    }
                ]),
            ),
            ChatMessage::tool_result("call_bash_99", "bash", "test result: ok"),
            ChatMessage::tool("fallback without id"),
        ];

        let openai_msgs = ProviderClient::format_openai_messages("System instruction", &messages);

        // First message must be system
        assert_eq!(openai_msgs[0]["role"], "system");
        assert_eq!(openai_msgs[0]["content"], "System instruction");

        // Second message: user
        assert_eq!(openai_msgs[1]["role"], "user");

        // Third message: assistant with tool_calls
        assert_eq!(openai_msgs[2]["role"], "assistant");
        assert!(openai_msgs[2]["tool_calls"].is_array());

        // Fourth message: tool with explicit tool_call_id
        assert_eq!(openai_msgs[3]["role"], "tool");
        assert_eq!(openai_msgs[3]["tool_call_id"], "call_bash_99");

        // Fifth message: tool without explicit id gets fallback
        assert_eq!(openai_msgs[4]["role"], "tool");
        assert_eq!(openai_msgs[4]["tool_call_id"], "call_unknown");
    }

    #[test]
    fn test_tool_formatting_helpers() {
        let tools = vec![
            serde_json::json!({
                "name": "read",
                "description": "Read file contents",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" }
                    },
                    "required": ["path"]
                }
            }),
            serde_json::json!({
                "name": "write",
                "description": "Write file",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" },
                        "content": { "type": "string" }
                    }
                }
            }),
        ];

        let ant_tools = ProviderClient::format_anthropic_tools(&tools);
        assert_eq!(ant_tools.len(), 2);
        assert_eq!(ant_tools[0]["name"], "read");
        assert!(ant_tools[0]["input_schema"]["properties"]["path"].is_object());
        assert_eq!(ant_tools[1]["name"], "write");
        assert!(ant_tools[1]["input_schema"]["properties"]["content"].is_object());
        assert_eq!(ant_tools[1]["cache_control"]["type"], "ephemeral");

        let openai_tools = ProviderClient::format_openai_tools(&tools);
        assert_eq!(openai_tools.len(), 2);
        assert_eq!(openai_tools[0]["type"], "function");
        assert_eq!(openai_tools[0]["function"]["name"], "read");
    }

    #[test]
    fn test_anthropic_prompt_caching_breakpoints() {
        // 1. Single message: caches initial turn
        let mut single = vec![serde_json::json!({
            "role": "user",
            "content": "Initial prompt"
        })];
        ProviderClient::apply_anthropic_prompt_caching(&mut single);
        assert_eq!(
            single[0]["content"][0]["cache_control"]["type"],
            "ephemeral"
        );
        assert_eq!(single[0]["content"][0]["text"], "Initial prompt");

        // 2. Multi-turn conversation (4 messages): caches initial turn and penultimate turn
        let mut multi = vec![
            serde_json::json!({
                "role": "user",
                "content": "Turn 1 user"
            }),
            serde_json::json!({
                "role": "assistant",
                "content": "Turn 1 assistant"
            }),
            serde_json::json!({
                "role": "user",
                "content": "Turn 2 user"
            }),
            serde_json::json!({
                "role": "assistant",
                "content": "Turn 2 assistant"
            }),
        ];
        ProviderClient::apply_anthropic_prompt_caching(&mut multi);

        // Breakpoint 1 on initial message (index 0)
        assert_eq!(multi[0]["content"][0]["cache_control"]["type"], "ephemeral");
        // Breakpoint 2 on penultimate turn (index 2 = multi.len() - 2)
        assert_eq!(multi[2]["content"][0]["cache_control"]["type"], "ephemeral");
        // Latest assistant turn (index 3) is untouched
        assert!(multi[3]["content"].is_string());
    }

    #[test]
    fn test_interleaved_parallel_tool_calls_openai_sse() {
        let lines = vec![
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_0\",\"function\":{\"name\":\"read\",\"arguments\":\"{\\\"path\\\":\"}}]}}]}\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":1,\"id\":\"call_1\",\"function\":{\"name\":\"edit\",\"arguments\":\"{\\\"path\\\":\"}}]}}]}\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"a.rs\\\"}\"}}]}}]}\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":1,\"function\":{\"arguments\":\"\\\"b.rs\\\"}\"}}]}}]}\n",
            "data: [DONE]\n",
        ];

        let mut state = OpenAiStreamState::new();
        for line in lines {
            state.process_line(line, |_| {});
        }
        let resp = state.finish();
        assert_eq!(resp.tool_calls.len(), 2);
        assert_eq!(resp.tool_calls[0].id, "call_0");
        assert_eq!(resp.tool_calls[0].name, "read");
        assert_eq!(
            resp.tool_calls[0].arguments,
            serde_json::json!({"path": "a.rs"})
        );
        assert_eq!(resp.tool_calls[1].id, "call_1");
        assert_eq!(resp.tool_calls[1].name, "edit");
        assert_eq!(
            resp.tool_calls[1].arguments,
            serde_json::json!({"path": "b.rs"})
        );
    }

    #[test]
    fn test_thinking_transition_directly_to_tool_use() {
        // OpenAI: reasoning_content followed directly by tool_calls
        let openai_lines = vec![
            "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"I should check file status.\"}}]}\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_git\",\"function\":{\"name\":\"git\",\"arguments\":\"{\\\"action\\\":\\\"status\\\"}\"}}]}}]}\n",
            "data: [DONE]\n",
        ];
        let mut chunks = Vec::new();
        let mut oai_state = OpenAiStreamState::new();
        for line in openai_lines {
            oai_state.process_line(line, |c| chunks.push(c));
        }
        let oai_resp = oai_state.finish();
        assert!(chunks.contains(&"<thinking>\n".to_string()));
        assert!(chunks.contains(&"\n</thinking>\n\n".to_string()));
        assert_eq!(oai_resp.tool_calls.len(), 1);
        assert_eq!(oai_resp.tool_calls[0].name, "git");

        // Anthropic: thinking_delta followed by content_block_stop then content_block_start tool_use
        let ant_lines = vec![
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\"}}\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"Analyzing project...\"}}\n",
            "event: content_block_stop\n",
            "data: {\"type\":\"content_block_stop\",\"index\":0}\n",
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_git\",\"name\":\"git\",\"input\":{}}}\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"action\\\":\\\"status\\\"}\"}}\n",
            "event: content_block_stop\n",
            "data: {\"type\":\"content_block_stop\",\"index\":1}\n",
        ];
        let mut ant_chunks = Vec::new();
        let mut ant_state = AnthropicStreamState::new();
        for line in ant_lines {
            ant_state.process_line(line, |c| ant_chunks.push(c));
        }
        let ant_resp = ant_state.finish();
        assert!(
            ant_resp
                .text
                .contains("<thinking>\nAnalyzing project...\n</thinking>\n\n")
        );
        assert_eq!(ant_resp.tool_calls.len(), 1);
        assert_eq!(ant_resp.tool_calls[0].name, "git");
    }

    #[test]
    fn test_malformed_sse_chunks_ignored() {
        let lines = vec![
            "",
            "   ",
            ": ping",
            ": keep-alive comment",
            "data:",
            "data:    ",
            "data: not valid json at all {{}}",
            "data: 12345",
            "data: {\"choices\": [{\"delta\": {\"content\": \"Valid after noise\"}}]}",
            "data: [DONE]",
        ];
        let mut chunks = Vec::new();
        let mut state = OpenAiStreamState::new();
        for line in lines {
            state.process_line(line, |c| chunks.push(c));
        }
        let resp = state.finish();
        assert_eq!(chunks, vec!["Valid after noise"]);
        assert_eq!(resp.text, "Valid after noise");
    }

    #[test]
    fn test_model_config_routing_and_limits() {
        // 1. OpenRouter with nested slash
        let or_cfg = ModelConfig::resolve("openrouter/deepseek/deepseek-r1:free");
        assert_eq!(or_cfg.provider, "openrouter");
        assert_eq!(or_cfg.model_id, "deepseek/deepseek-r1:free");
        assert_eq!(or_cfg.context_window, 64_000);

        // 2. Gemini 2M
        let gemini_cfg = ModelConfig::resolve("gemini/gemini-1.5-pro");
        assert_eq!(gemini_cfg.provider, "gemini");
        assert_eq!(gemini_cfg.model_id, "gemini-1.5-pro");
        assert_eq!(gemini_cfg.context_window, 2_097_152);

        // 3. Claude 3.7 Sonnet
        let claude_cfg = ModelConfig::resolve("anthropic/claude-3-7-sonnet-latest");
        assert_eq!(claude_cfg.provider, "anthropic");
        assert_eq!(claude_cfg.model_id, "claude-3-7-sonnet-latest");
        assert_eq!(claude_cfg.context_window, 200_000);
        assert_eq!(claude_cfg.max_output, 64_000);

        // 4. Codestral 256k
        let code_cfg = ModelConfig::resolve("mistral/codestral-latest");
        assert_eq!(code_cfg.provider, "mistral");
        assert_eq!(code_cfg.model_id, "codestral-latest");
        assert_eq!(code_cfg.context_window, 256_000);

        // 5. Local Ollama
        let ollama_cfg = ModelConfig::resolve("ollama/qwen2.5-coder:32b");
        assert_eq!(ollama_cfg.provider, "ollama");
        assert_eq!(ollama_cfg.model_id, "qwen2.5-coder:32b");
        assert_eq!(ollama_cfg.context_window, 128_000);
    }
}
