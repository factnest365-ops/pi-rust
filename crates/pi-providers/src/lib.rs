use anyhow::Result;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

pub mod auth;
pub mod models;
pub mod tokens;
pub use auth::{AuthCredential, AuthResolver, AuthStore};
pub use models::{ModelCatalogLoader, ModelInfo};
pub use tokens::{ContextBudget, TokenProfiler};

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
    #[serde(flatten)]
    pub custom_keys: std::collections::BTreeMap<String, serde_json::Value>,
}

impl PiConfig {
    fn config_path() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let pi_dir = home.join(".pi");
        let _ = fs::create_dir_all(&pi_dir);
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
            .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).map_err(anyhow::Error::from))
        {
            if cfg.opencode_api_key.is_none() {
                cfg.opencode_api_key = json.get("opencode")
                    .and_then(|v| v.get("key"))
                    .and_then(|k| k.as_str())
                    .map(ToString::to_string);
            }
            if cfg.kilo_api_key.is_none() {
                cfg.kilo_api_key = json.get("kilo")
                    .and_then(|v| v.get("key"))
                    .and_then(|k| k.as_str())
                    .map(ToString::to_string);
            }
            if cfg.anthropic_api_key.is_none() {
                cfg.anthropic_api_key = json.get("anthropic")
                    .and_then(|v| v.get("key"))
                    .and_then(|k| k.as_str())
                    .map(ToString::to_string);
            }
            if cfg.openai_api_key.is_none() {
                cfg.openai_api_key = json.get("openai")
                    .and_then(|v| v.get("key"))
                    .and_then(|k| k.as_str())
                    .map(ToString::to_string);
            }
            if cfg.gemini_api_key.is_none() {
                cfg.gemini_api_key = json.get("gemini")
                    .or_else(|| json.get("google"))
                    .and_then(|v| v.get("key"))
                    .and_then(|k| k.as_str())
                    .map(ToString::to_string);
            }
            if cfg.openrouter_api_key.is_none() {
                cfg.openrouter_api_key = json.get("openrouter")
                    .and_then(|v| v.get("key"))
                    .and_then(|k| k.as_str())
                    .map(ToString::to_string);
            }
            if cfg.deepseek_api_key.is_none() {
                cfg.deepseek_api_key = json.get("deepseek")
                    .and_then(|v| v.get("key"))
                    .and_then(|k| k.as_str())
                    .map(ToString::to_string);
            }
            if cfg.groq_api_key.is_none() {
                cfg.groq_api_key = json.get("groq")
                    .and_then(|v| v.get("key"))
                    .and_then(|k| k.as_str())
                    .map(ToString::to_string);
            }
            if cfg.cerebras_api_key.is_none() {
                cfg.cerebras_api_key = json.get("cerebras")
                    .and_then(|v| v.get("key"))
                    .and_then(|k| k.as_str())
                    .map(ToString::to_string);
            }
            if cfg.mistral_api_key.is_none() {
                cfg.mistral_api_key = json.get("mistral")
                    .and_then(|v| v.get("key"))
                    .and_then(|k| k.as_str())
                    .map(ToString::to_string);
            }
        }

        cfg
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
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
                self.custom_keys.insert(format!("{}_api_key", norm), serde_json::Value::String(key));
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

        // Scan local Ollama daemon
        if let Ok(res) = client
            .get("http://localhost:11434/api/tags")
            .timeout(std::time::Duration::from_millis(500))
            .send()
            .await
            && res.status().is_success()
            && let Ok(json) = res.json::<serde_json::Value>().await
            && let Some(models) = json.get("models").and_then(|m| m.as_array())
        {
            for m in models {
                if let Some(name) = m.get("name").and_then(|n| n.as_str()) {
                    list.push(format!("ollama/{}", name));
                }
            }
        }

        // Scan local llama.cpp daemon
        if let Ok(res) = client
            .get("http://localhost:8080/v1/models")
            .timeout(std::time::Duration::from_millis(500))
            .send()
            .await
            && res.status().is_success()
            && let Ok(json) = res.json::<serde_json::Value>().await
            && let Some(data) = json.get("data").and_then(|d| d.as_array())
        {
            for m in data {
                if let Some(id) = m.get("id").and_then(|n| n.as_str()) {
                    list.push(format!("llamacpp/{}", id));
                }
            }
        }

        // Scan local LM Studio daemon
        if let Ok(res) = client
            .get("http://localhost:1234/v1/models")
            .timeout(std::time::Duration::from_millis(500))
            .send()
            .await
            && res.status().is_success()
            && let Ok(json) = res.json::<serde_json::Value>().await
            && let Some(data) = json.get("data").and_then(|d| d.as_array())
        {
            for m in data {
                if let Some(id) = m.get("id").and_then(|n| n.as_str()) {
                    list.push(format!("lmstudio/{}", id));
                }
            }
        }

        // Scan local vLLM daemon
        if let Ok(res) = client
            .get("http://localhost:8000/v1/models")
            .timeout(std::time::Duration::from_millis(500))
            .send()
            .await
            && res.status().is_success()
            && let Ok(json) = res.json::<serde_json::Value>().await
            && let Some(data) = json.get("data").and_then(|d| d.as_array())
        {
            for m in data {
                if let Some(id) = m.get("id").and_then(|n| n.as_str()) {
                    list.push(format!("vllm/{}", id));
                }
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
        let (provider, model_id) = if let Some((p, m)) = raw_model.split_once('/') {
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
        } else if raw_model.contains("openai") || raw_model.starts_with("gpt-") || raw_model.starts_with("o1") || raw_model.starts_with("o3") {
            ("openai", raw_model)
        } else if raw_model.contains("agnes") {
            ("agnes", raw_model)
        } else if raw_model.contains("ollama") {
            ("ollama", raw_model)
        } else if raw_model.contains("llamacpp") || raw_model.contains("llama.cpp") {
            ("llamacpp", raw_model)
        } else if raw_model.contains("lmstudio") {
            ("lmstudio", raw_model)
        } else {
            ("opencode", raw_model)
        };

        let mut resolved_key = AuthResolver::resolve_key(provider).unwrap_or_default();
        if resolved_key.is_empty() && (provider == "opencode" || provider == "openrouter") {
            // Free tier models work with empty or public bearer token
            resolved_key = String::new();
        }

        let (api_key, base_url) = match provider {
            "opencode" | "opencode-zen" | "zen" => (
                resolved_key,
                Some(
                    std::env::var("OPENCODE_BASE_URL")
                        .ok()
                        .or_else(|| Self::lookup_models_json_base_url("opencode"))
                        .unwrap_or_else(|| "https://opencode.ai/zen/v1".to_string()),
                ),
            ),
            "agnes" | "agnes-gateway" => (
                resolved_key,
                Some(
                    std::env::var("AGNES_BASE_URL")
                        .ok()
                        .or_else(|| Self::lookup_models_json_base_url("agnes"))
                        .unwrap_or_else(|| "https://api.agnes.ai/v1".to_string()),
                ),
            ),
            "kilo" | "kilo-gateway" => (
                resolved_key,
                Some(
                    std::env::var("KILO_BASE_URL")
                        .ok()
                        .or_else(|| Self::lookup_models_json_base_url("kilo"))
                        .unwrap_or_else(|| "https://api.kilo.ai/api/gateway".to_string()),
                ),
            ),
            "ollama" => (
                "ollama".to_string(),
                Some("http://localhost:11434/v1".to_string()),
            ),
            "llamacpp" | "llama.cpp" => (
                "llamacpp".to_string(),
                Some("http://localhost:8080/v1".to_string()),
            ),
            "lmstudio" => (
                "lmstudio".to_string(),
                Some("http://localhost:1234/v1".to_string()),
            ),
            "vllm" => (
                "vllm".to_string(),
                Some("http://localhost:8000/v1".to_string()),
            ),
            "anthropic" => (
                resolved_key,
                Some("https://api.anthropic.com/v1".to_string()),
            ),
            "openai" => (
                resolved_key,
                Some("https://api.openai.com/v1".to_string()),
            ),
            "gemini" | "google" => (
                resolved_key,
                Some("https://generativelanguage.googleapis.com/v1beta/openai".to_string()),
            ),
            "openrouter" => (
                resolved_key,
                Some("https://openrouter.ai/api/v1".to_string()),
            ),
            "deepseek" => (
                resolved_key,
                Some("https://api.deepseek.com/v1".to_string()),
            ),
            "groq" => (
                resolved_key,
                Some("https://api.groq.com/openai/v1".to_string()),
            ),
            "cerebras" => (
                resolved_key,
                Some("https://api.cerebras.ai/v1".to_string()),
            ),
            "mistral" => (
                resolved_key,
                Some("https://api.mistral.ai/v1".to_string()),
            ),
            "xai" => (
                resolved_key,
                Some("https://api.x.ai/v1".to_string()),
            ),
            "together" | "together-ai" => (
                resolved_key,
                Some("https://api.together.xyz/v1".to_string()),
            ),
            "fireworks" => (
                resolved_key,
                Some("https://api.fireworks.ai/inference/v1".to_string()),
            ),
            "perplexity" => (
                resolved_key,
                Some("https://api.perplexity.ai".to_string()),
            ),
            "copilot" | "github-copilot" => (
                resolved_key,
                Some("https://api.githubcopilot.com".to_string()),
            ),
            "qwen" | "qwen-token-plan" => (
                resolved_key,
                Some("https://dashscope-intl.aliyuncs.com/compatible-mode/v1".to_string()),
            ),
            "xiaomi" | "mimo" => (
                resolved_key,
                Some("https://api.mimo.xiaomi.com/v1".to_string()),
            ),
            "moonshot" | "kimi" => (
                resolved_key,
                Some("https://api.moonshot.cn/v1".to_string()),
            ),
            "huggingface" | "hf" => (
                resolved_key,
                Some("https://api-inference.huggingface.co/v1".to_string()),
            ),
            _ => (
                resolved_key,
                Some("https://api.kilo.ai/v1".to_string()),
            ),
        };

        Self {
            provider: provider.to_string(),
            model_id: model_id.to_string(),
            api_key,
            base_url,
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

    pub fn assistant_with_tool_calls(content: impl Into<String>, tool_calls: serde_json::Value) -> Self {
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

    pub fn tool_result(tool_call_id: impl Into<String>, name: impl Into<String>, content: impl Into<String>) -> Self {
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
                    if let Some(tool_calls_arr) = delta.get("tool_calls").and_then(|t| t.as_array()) {
                        for tc in tool_calls_arr {
                            let index = tc.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                            let entry = self.tool_calls.entry(index).or_default();
                            if let Some(id) = tc.get("id").and_then(|i| i.as_str())
                                && !id.is_empty()
                            {
                                entry.id = id.to_string();
                            }
                            if let Some(name) = tc.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str())
                                && !name.is_empty()
                                && entry.name.is_empty()
                            {
                                entry.name = name.to_string();
                            }
                            if let Some(args) = tc.get("function").and_then(|f| f.get("arguments")).and_then(|a| a.as_str()) {
                                entry.arguments_buf.push_str(args);
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
                        let id = cb.get("id").and_then(|i| i.as_str()).unwrap_or("call_unknown").to_string();
                        let name = cb.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                        let entry = self.tool_calls.entry(index).or_default();
                        entry.id = id;
                        entry.name = name;
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
                            if let Some(partial) = delta.get("partial_json").and_then(|p| p.as_str()) {
                                let entry = self.tool_calls.entry(index).or_default();
                                entry.arguments_buf.push_str(partial);
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
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

pub struct ProviderClient;

impl ProviderClient {
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
            // Anthropic Messages API requires alternating user/assistant roles with structured tool_use and tool_result blocks.
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
                } else if msg.role == "assistant" && let Some(ref tc) = msg.tool_calls && let Some(tc_arr) = tc.as_array() {
                    let mut content_blocks = Vec::new();
                    if !msg.content.is_empty() {
                        content_blocks.push(serde_json::json!({
                            "type": "text",
                            "text": msg.content
                        }));
                    }
                    for call in tc_arr {
                        let id = call.get("id").and_then(|i| i.as_str()).unwrap_or("toolu_unknown");
                        let name = call.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()).unwrap_or("");
                        let args_str = call.get("function").and_then(|f| f.get("arguments")).and_then(|a| a.as_str()).unwrap_or("{}");
                        let parsed_args: serde_json::Value = serde_json::from_str(args_str).unwrap_or_else(|_| serde_json::json!({}));
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
                        && let Some(prev_text) = last["content"].as_str()
                    {
                        last["content"] = serde_json::json!(format!("{}\n\n{}", prev_text, msg.content));
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

            let mut body = serde_json::json!({
                "model": config.model_id,
                "max_tokens": 4096,
                "system": system_prompt,
                "messages": anthropic_messages,
                "stream": true,
            });

            if !tools.is_empty() {
                let anthropic_tools: Vec<serde_json::Value> = tools
                    .iter()
                    .map(|t| {
                        serde_json::json!({
                            "name": t["name"],
                            "description": t["description"],
                            "input_schema": t["parameters"]
                        })
                    })
                    .collect();
                body["tools"] = serde_json::Value::Array(anthropic_tools);
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
                .header("content-type", "application/json");

            if !config.api_key.is_empty() {
                req = req.header("x-api-key", &config.api_key);
            }

            let res = req.json(&body).send().await?;
            let status = res.status();
            if !status.is_success() {
                let err_text = res.text().await.unwrap_or_default();
                return Err(anyhow::anyhow!("Anthropic API error ({}): {}", status, err_text));
            }

            let mut stream = res.bytes_stream();
            let mut byte_buf = Vec::new();
            let mut state = AnthropicStreamState::new();

            while let Some(chunk_res) = stream.next().await {
                let chunk = chunk_res?;
                byte_buf.extend_from_slice(&chunk);

                while let Some(pos) = byte_buf.iter().position(|&b| b == b'\n') {
                    let line_bytes = &byte_buf[..pos];
                    let line_str = String::from_utf8_lossy(line_bytes).trim_end_matches('\r').to_string();
                    byte_buf.drain(..=pos);
                    state.process_line(&line_str, &mut on_chunk);
                }
            }

            if !byte_buf.is_empty() {
                let line_str = String::from_utf8_lossy(&byte_buf).trim_end_matches('\r').to_string();
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

            let mut body = serde_json::json!({
                "model": config.model_id,
                "messages": openai_messages,
                "stream": true,
            });

            if !tools.is_empty() {
                let openai_tools: Vec<serde_json::Value> = tools
                    .iter()
                    .map(|t| {
                        serde_json::json!({
                            "type": "function",
                            "function": t
                        })
                    })
                    .collect();
                body["tools"] = serde_json::Value::Array(openai_tools);
            }

            let mut req = client.post(&endpoint);
            if !config.api_key.is_empty() {
                req = req.bearer_auth(&config.api_key);
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
                    let line_str = String::from_utf8_lossy(line_bytes).trim_end_matches('\r').to_string();
                    byte_buf.drain(..=pos);
                    state.process_line(&line_str, &mut on_chunk);
                }
            }

            if !byte_buf.is_empty() {
                let line_str = String::from_utf8_lossy(&byte_buf).trim_end_matches('\r').to_string();
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
        assert_eq!(opencode.base_url.as_deref(), Some("https://opencode.ai/zen/v1"));

        let kilo = ModelConfig::resolve("kilo/deepseek-r1");
        assert_eq!(kilo.provider, "kilo");
        assert_eq!(kilo.model_id, "deepseek-r1");
        assert_eq!(kilo.base_url.as_deref(), Some("https://api.kilo.ai/api/gateway"));

        let agnes = ModelConfig::resolve("agnes/agnes-core");
        assert_eq!(agnes.provider, "agnes");
        assert_eq!(agnes.model_id, "agnes-core");
        assert_eq!(agnes.base_url.as_deref(), Some("https://api.agnes.ai/v1"));

        let ollama = ModelConfig::resolve("ollama/llama3");
        assert_eq!(ollama.provider, "ollama");
        assert_eq!(ollama.model_id, "llama3");
        assert_eq!(ollama.api_key, "ollama");
        assert_eq!(ollama.base_url.as_deref(), Some("http://localhost:11434/v1"));

        let llamacpp = ModelConfig::resolve("llamacpp/llama-3.2-3b");
        assert_eq!(llamacpp.provider, "llamacpp");
        assert_eq!(llamacpp.model_id, "llama-3.2-3b");
        assert_eq!(llamacpp.api_key, "llamacpp");
        assert_eq!(llamacpp.base_url.as_deref(), Some("http://localhost:8080/v1"));

        let lmstudio = ModelConfig::resolve("lmstudio/mistral-7b");
        assert_eq!(lmstudio.provider, "lmstudio");
        assert_eq!(lmstudio.model_id, "mistral-7b");
        assert_eq!(lmstudio.api_key, "lmstudio");
        assert_eq!(lmstudio.base_url.as_deref(), Some("http://localhost:1234/v1"));

        // Fallbacks without slash
        let ollama_fallback = ModelConfig::resolve("local-ollama-llama3");
        assert_eq!(ollama_fallback.provider, "ollama");
        assert_eq!(ollama_fallback.api_key, "ollama");
        assert_eq!(ollama_fallback.base_url.as_deref(), Some("http://localhost:11434/v1"));

        let llamacpp_fallback = ModelConfig::resolve("llamacpp-qwen");
        assert_eq!(llamacpp_fallback.provider, "llamacpp");
        assert_eq!(llamacpp_fallback.api_key, "llamacpp");
        assert_eq!(llamacpp_fallback.base_url.as_deref(), Some("http://localhost:8080/v1"));

        let lmstudio_fallback = ModelConfig::resolve("lmstudio-hermes");
        assert_eq!(lmstudio_fallback.provider, "lmstudio");
        assert_eq!(lmstudio_fallback.api_key, "lmstudio");
        assert_eq!(lmstudio_fallback.base_url.as_deref(), Some("http://localhost:1234/v1"));
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
        assert_eq!(chunks, vec!["<thinking>\n", "Thinking step 1... ", "\n</thinking>\n\n", "Final answer."]);
        assert_eq!(resp.text, "<thinking>\nThinking step 1... \n</thinking>\n\nFinal answer.");

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
        assert_eq!(ant_chunks, vec!["<thinking>\n", "Reasoning about code... ", "\n</thinking>\n\n", "Here is the solution."]);
        assert_eq!(ant_resp.text, "<thinking>\nReasoning about code... \n</thinking>\n\nHere is the solution.");
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
}
