use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn get_http_client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_default()
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub context_window: usize,
    pub max_output: usize,
    pub supports_reasoning: bool,
    pub supports_vision: bool,
    pub description: String,
}

impl ModelInfo {
    pub fn new(id: &str, provider: &str, context_window: usize, max_output: usize, reasoning: bool, vision: bool, desc: &str) -> Self {
        Self {
            id: id.to_string(),
            name: id.split('/').next_back().unwrap_or(id).to_string(),
            provider: provider.to_string(),
            context_window,
            max_output,
            supports_reasoning: reasoning,
            supports_vision: vision,
            description: desc.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCatalogData {
    pub last_refreshed: String,
    pub models: Vec<ModelInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CustomModelsConfig {
    pub providers: Option<std::collections::BTreeMap<String, CustomProviderConfig>>,
    pub models: Option<Vec<CustomModelEntry>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CustomProviderConfig {
    #[serde(rename = "baseUrl")]
    pub base_url: Option<String>,
    #[serde(rename = "apiKey")]
    pub api_key: Option<String>,
    pub models: Option<Vec<CustomModelEntry>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CustomModelEntry {
    pub id: String,
    pub name: Option<String>,
    pub provider: Option<String>,
    #[serde(rename = "contextWindow")]
    pub context_window: Option<usize>,
    #[serde(rename = "maxOutput")]
    pub max_output: Option<usize>,
    pub reasoning: Option<bool>,
    pub vision: Option<bool>,
    pub description: Option<String>,
}

pub struct ModelCatalogLoader;

impl ModelCatalogLoader {
    fn cache_path() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let pi_dir = home.join(".pi");
        let _ = fs::create_dir_all(&pi_dir);
        pi_dir.join("models_cache.json")
    }

    /// Curated list of all latest frontier, gateway, cloud, and specialized models
    pub fn static_frontier_models() -> Vec<ModelInfo> {
        vec![
            // Anthropic Claude
            ModelInfo::new("anthropic/claude-3-7-sonnet-latest", "Anthropic", 200_000, 64_000, true, true, "Most intelligent Claude model with hybrid reasoning & coding"),
            ModelInfo::new("anthropic/claude-3-5-sonnet-latest", "Anthropic", 200_000, 8_192, false, true, "Industry benchmark for high-performance agentic coding"),
            ModelInfo::new("anthropic/claude-3-5-haiku-latest", "Anthropic", 200_000, 8_192, false, false, "Ultra-fast low-latency code assistant"),
            ModelInfo::new("anthropic/claude-3-opus-latest", "Anthropic", 200_000, 4_096, false, true, "Deep analysis and complex reasoning"),

            // OpenAI Frontier
            ModelInfo::new("openai/gpt-4.5-preview", "OpenAI", 128_000, 16_384, false, true, "Massive world-knowledge frontier model"),
            ModelInfo::new("openai/gpt-4o", "OpenAI", 128_000, 16_384, false, true, "Flagship versatile multimodal intelligence"),
            ModelInfo::new("openai/gpt-4o-mini", "OpenAI", 128_000, 16_384, false, true, "Fast, lightweight multimodal model"),
            ModelInfo::new("openai/o1", "OpenAI", 200_000, 100_000, true, true, "Full deep reasoning model for hard STEM & architecture"),
            ModelInfo::new("openai/o3-mini", "OpenAI", 200_000, 100_000, true, false, "Fast reasoning and code synthesis model"),

            // OpenRouter Free Tier Models (Zero Cost)
            ModelInfo::new("openrouter/deepseek/deepseek-r1:free", "OpenRouter (Free)", 64_000, 8_000, true, false, "Free DeepSeek-R1 full open reasoning model"),
            ModelInfo::new("openrouter/deepseek/deepseek-chat:free", "OpenRouter (Free)", 64_000, 8_000, false, false, "Free DeepSeek-V3 671B MoE model"),
            ModelInfo::new("openrouter/meta-llama/llama-3.3-70b-instruct:free", "OpenRouter (Free)", 128_000, 8_192, false, false, "Free Llama 3.3 70B flagship open model"),
            ModelInfo::new("openrouter/google/gemini-2.0-flash-exp:free", "OpenRouter (Free)", 1_048_576, 8_192, false, true, "Free Gemini 2.0 Flash Experimental with 1M context"),
            ModelInfo::new("openrouter/google/gemini-2.0-flash-thinking-exp:free", "OpenRouter (Free)", 1_048_576, 8_192, true, true, "Free Gemini 2.0 Flash Thinking Experimental reasoning"),
            ModelInfo::new("openrouter/qwen/qwen-2.5-coder-32b-instruct:free", "OpenRouter (Free)", 128_000, 8_192, false, false, "Free Qwen 2.5 Coder 32B specialized coding model"),
            ModelInfo::new("openrouter/mistralai/mistral-small-24b-instruct-2501:free", "OpenRouter (Free)", 32_000, 8_192, false, false, "Free Mistral Small 24B lightweight model"),

            // Google Gemini & Free Experimental Models
            ModelInfo::new("gemini/gemini-2.0-flash", "Google", 1_048_576, 8_192, false, true, "Next-gen multimodal flash speed with 1M context"),
            ModelInfo::new("gemini/gemini-2.0-flash-exp", "Google (Free Exp)", 1_048_576, 8_192, false, true, "Gemini 2.0 Flash Experimental with 1M context"),
            ModelInfo::new("gemini/gemini-2.0-flash-thinking-exp", "Google (Free Exp)", 1_048_576, 64_000, true, true, "Gemini 2.0 Flash Thinking Experimental reasoning"),
            ModelInfo::new("gemini/gemini-exp-1206", "Google (Free Exp)", 2_097_152, 8_192, true, true, "Gemini Quality Experimental 1206 with 2M context"),
            ModelInfo::new("gemini/gemini-2.0-pro-exp", "Google", 2_097_152, 8_192, true, true, "2M context frontier reasoning and coding"),
            ModelInfo::new("gemini/gemini-1.5-pro", "Google", 2_097_152, 8_192, false, true, "Ultra-long 2M context comprehension"),

            // Ollama / Local Models (Zero Cost / Free Local Daemons)
            ModelInfo::new("ollama/qwen2.5-coder:32b", "Ollama (Local Free)", 128_000, 16_384, false, false, "Local Qwen 2.5 Coder 32B running via Ollama"),
            ModelInfo::new("ollama/deepseek-r1:32b", "Ollama (Local Free)", 64_000, 8_000, true, false, "Local DeepSeek R1 32B reasoning running via Ollama"),
            ModelInfo::new("ollama/llama3.3:70b", "Ollama (Local Free)", 128_000, 8_192, false, false, "Local Llama 3.3 70B running via Ollama"),
            ModelInfo::new("lmstudio/local-model", "LM Studio (Local Free)", 128_000, 8_192, false, true, "Local LLM loaded in LM Studio on port 1234"),

            // DeepSeek
            ModelInfo::new("deepseek/deepseek-chat", "DeepSeek", 64_000, 8_000, false, false, "DeepSeek-V3 671B MoE frontier coding"),
            ModelInfo::new("deepseek/deepseek-reasoner", "DeepSeek", 64_000, 8_000, true, false, "DeepSeek-R1 open reasoning model"),

            // GitHub Copilot Gateway
            ModelInfo::new("copilot/claude-3.5-sonnet", "GitHub Copilot", 200_000, 8_192, false, true, "GitHub Copilot Claude 3.5 Sonnet pipeline"),
            ModelInfo::new("copilot/gpt-4o", "GitHub Copilot", 128_000, 8_192, false, true, "GitHub Copilot GPT-4o pipeline"),
            ModelInfo::new("copilot/o3-mini", "GitHub Copilot", 200_000, 100_000, true, false, "GitHub Copilot o3-mini reasoning"),

            // Amazon Bedrock Gateway
            ModelInfo::new("bedrock/anthropic.claude-3-5-sonnet-20241022-v2:0", "Amazon Bedrock", 200_000, 8_192, false, true, "Amazon Bedrock Claude 3.5 Sonnet"),
            ModelInfo::new("bedrock/anthropic.claude-3-5-haiku-20241022-v1:0", "Amazon Bedrock", 200_000, 8_192, false, false, "Amazon Bedrock Claude 3.5 Haiku"),

            // Groq High-Throughput
            ModelInfo::new("groq/llama-3.3-70b-versatile", "Groq", 128_000, 32_768, false, false, "Llama 3.3 70B running at 300+ tok/s"),
            ModelInfo::new("groq/deepseek-r1-distill-llama-70b", "Groq", 128_000, 8_192, true, false, "High-speed DeepSeek-R1 distillation"),

            // Mistral & Codestral
            ModelInfo::new("mistral/mistral-large-latest", "Mistral", 128_000, 8_192, false, false, "Flagship European multilingual model"),
            ModelInfo::new("mistral/codestral-latest", "Mistral", 256_000, 8_192, false, false, "Specialized coding LLM with 256k context"),

            // Cerebras Ultra-Low Latency
            ModelInfo::new("cerebras/llama-3.3-70b", "Cerebras", 128_000, 8_192, false, false, "Ultra-fast Cerebras CS-3 wafer engine"),

            // Perplexity
            ModelInfo::new("perplexity/sonar-reasoning-pro", "Perplexity", 128_000, 8_192, true, false, "Deep web-grounded reasoning model"),

            // Together AI & Fireworks
            ModelInfo::new("together/meta-llama/Llama-3.3-70B-Instruct-Turbo", "Together AI", 128_000, 8_192, false, false, "Fast open Llama 3.3"),
            ModelInfo::new("fireworks/accounts/fireworks/models/deepseek-r1", "Fireworks", 64_000, 8_000, true, false, "Serverless DeepSeek-R1 reasoning"),
        ]
    }

    /// Load user-defined model overrides from ~/.pi/agent/models.json and ~/.pi/models.json
    pub fn load_user_overrides() -> Vec<ModelInfo> {
        let mut custom_models = Vec::new();
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

        let paths = vec![
            home.join(".pi").join("agent").join("models.json"),
            home.join(".pi").join("models.json"),
        ];

        for path in paths {
            if let Ok(content) = fs::read_to_string(&path)
                && let Ok(cfg) = serde_json::from_str::<CustomModelsConfig>(&content)
            {
                if let Some(providers) = cfg.providers {
                    for (prov_name, p_cfg) in providers {
                        if let Some(models) = p_cfg.models {
                            for m in models {
                                let id = if m.id.contains('/') { m.id.clone() } else { format!("{}/{}", prov_name, m.id) };
                                custom_models.push(ModelInfo::new(
                                    &id,
                                    m.provider.as_deref().unwrap_or(&prov_name),
                                    m.context_window.unwrap_or(128_000),
                                    m.max_output.unwrap_or(8_192),
                                    m.reasoning.unwrap_or(false),
                                    m.vision.unwrap_or(false),
                                    m.description.as_deref().unwrap_or("Custom user-configured model"),
                                ));
                            }
                        }
                    }
                }

                if let Some(models) = cfg.models {
                    for m in models {
                        let prov = m.provider.clone().unwrap_or_else(|| "Custom".to_string());
                        let id = m.id.clone();
                        custom_models.push(ModelInfo::new(
                            &id,
                            &prov,
                            m.context_window.unwrap_or(128_000),
                            m.max_output.unwrap_or(8_192),
                            m.reasoning.unwrap_or(false),
                            m.vision.unwrap_or(false),
                            m.description.as_deref().unwrap_or("Custom user-configured model"),
                        ));
                    }
                }
            }
        }

        custom_models
    }

    /// Loads instantly from local disk cache if available, falling back to static frontier catalog
    pub fn load_cached_or_static() -> Vec<ModelInfo> {
        let path = Self::cache_path();
        if let Ok(content) = fs::read_to_string(&path)
            && let Ok(catalog) = serde_json::from_str::<ModelCatalogData>(&content)
            && !catalog.models.is_empty()
        {
            return catalog.models;
        }
        Self::static_frontier_models()
    }

    /// Loads models from cache or queries live endpoints from all providers & local daemons
    pub async fn fetch_all_models(force_refresh: bool) -> Vec<ModelInfo> {
        let path = Self::cache_path();

        if !force_refresh
            && let Ok(content) = fs::read_to_string(&path)
            && let Ok(catalog) = serde_json::from_str::<ModelCatalogData>(&content)
            && let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&catalog.last_refreshed)
        {
            let age = chrono::Utc::now().signed_duration_since(ts);
            if age.num_hours() < 12 && !catalog.models.is_empty() {
                return catalog.models;
            }
        }

        let mut models = Self::static_frontier_models();

        // Incorporate custom user overrides from ~/.pi/agent/models.json
        for user_m in Self::load_user_overrides() {
            if let Some(pos) = models.iter().position(|m| m.id == user_m.id) {
                models[pos] = user_m;
            } else {
                models.push(user_m);
            }
        }

        let client = get_http_client();

        // 1. Query OpenRouter API (Live search across 250+ cloud & free models)
        if let Ok(res) = client.get("https://openrouter.ai/api/v1/models").send().await
            && res.status().is_success()
            && let Ok(json) = res.json::<serde_json::Value>().await
            && let Some(arr) = json.get("data").and_then(|d| d.as_array())
        {
            for item in arr.iter().take(150) {
                if let Some(raw_id) = item.get("id").and_then(|n| n.as_str()) {
                    let id = format!("openrouter/{}", raw_id);
                    let name = item.get("name").and_then(|n| n.as_str()).unwrap_or(raw_id);
                    let context = item.get("context_length").and_then(|c| c.as_u64()).unwrap_or(128_000) as usize;
                    let desc = item.get("description").and_then(|d| d.as_str()).unwrap_or("");

                    let is_free = raw_id.ends_with(":free")
                        || item.get("pricing").and_then(|p| p.get("prompt")).and_then(|pr| pr.as_str()) == Some("0");
                    let provider_name = if is_free { "OpenRouter (Free)" } else { "OpenRouter" };

                    if let Some(pos) = models.iter().position(|m| m.id == id) {
                        models[pos].context_window = context;
                    } else {
                        models.push(ModelInfo::new(
                            &id,
                            provider_name,
                            context,
                            8_192,
                            raw_id.contains("r1") || raw_id.contains("reason") || raw_id.contains("thinking"),
                            raw_id.contains("vision") || raw_id.contains("vl") || raw_id.contains("4o"),
                            if is_free { format!("[Free Tier] {}", name) } else if desc.is_empty() { name.to_string() } else { desc.to_string() }.as_str(),
                        ));
                    }
                }
            }
        }

        // 2. Query OpenCode Zen API (Live public endpoint with genuine free & frontier coding models)
        if let Ok(res) = client.get("https://opencode.ai/zen/v1/models").send().await
            && res.status().is_success()
            && let Ok(json) = res.json::<serde_json::Value>().await
            && let Some(arr) = json.get("data").and_then(|d| d.as_array())
        {
            for item in arr {
                if let Some(raw_id) = item.get("id").and_then(|n| n.as_str()) {
                    let id = format!("opencode/{}", raw_id);
                    let is_free = raw_id.ends_with("-free") || raw_id == "big-pickle";
                    let provider_name = if is_free { "OpenCode Zen (Free)" } else { "OpenCode Zen" };

                    if !models.iter().any(|m| m.id == id) {
                        models.push(ModelInfo::new(
                            &id,
                            provider_name,
                            128_000,
                            16_384,
                            raw_id.contains("r1") || raw_id.contains("pro") || raw_id.contains("ultra") || raw_id.contains("codex"),
                            raw_id.contains("sonnet") || raw_id.contains("flash") || raw_id.contains("opus"),
                            if is_free { format!("[Free Coding Model] OpenCode Zen: {}", raw_id) } else { format!("OpenCode Zen: {}", raw_id) }.as_str(),
                        ));
                    }
                }
            }
        }

        // 3. Query Local Ollama daemon (localhost:11434)
        if let Ok(res) = client.get("http://localhost:11434/api/tags").send().await
            && res.status().is_success()
            && let Ok(json) = res.json::<serde_json::Value>().await
            && let Some(arr) = json.get("models").and_then(|m| m.as_array())
        {
            for item in arr {
                if let Some(name) = item.get("name").and_then(|n| n.as_str()) {
                    let id = format!("ollama/{}", name);
                    if !models.iter().any(|m| m.id == id) {
                        models.push(ModelInfo::new(
                            &id,
                            "Ollama (Local Free)",
                            32_000,
                            4_096,
                            name.contains("r1") || name.contains("reason"),
                            name.contains("vision") || name.contains("llava"),
                            &format!("Local Ollama model: {}", name),
                        ));
                    }
                }
            }
        }

        // 3. Query Local LM Studio daemon (localhost:1234)
        if let Ok(res) = client.get("http://localhost:1234/v1/models").send().await
            && res.status().is_success()
            && let Ok(json) = res.json::<serde_json::Value>().await
            && let Some(arr) = json.get("data").and_then(|d| d.as_array())
        {
            for item in arr {
                if let Some(id_str) = item.get("id").and_then(|n| n.as_str()) {
                    let id = format!("lmstudio/{}", id_str);
                    if !models.iter().any(|m| m.id == id) {
                        models.push(ModelInfo::new(
                            &id,
                            "LM Studio (Local Free)",
                            32_000,
                            4_096,
                            id_str.contains("r1"),
                            false,
                            &format!("Local LM Studio model: {}", id_str),
                        ));
                    }
                }
            }
        }

        // 4. Query Local llama.cpp daemon (localhost:8080)
        if let Ok(res) = client.get("http://localhost:8080/v1/models").send().await
            && res.status().is_success()
            && let Ok(json) = res.json::<serde_json::Value>().await
            && let Some(arr) = json.get("data").and_then(|d| d.as_array())
        {
            for item in arr {
                if let Some(id_str) = item.get("id").and_then(|n| n.as_str()) {
                    let id = format!("llamacpp/{}", id_str);
                    if !models.iter().any(|m| m.id == id) {
                        models.push(ModelInfo::new(
                            &id,
                            "llama.cpp (Local Free)",
                            32_000,
                            4_096,
                            false,
                            false,
                            &format!("Local llama.cpp model: {}", id_str),
                        ));
                    }
                }
            }
        }

        // 5. Query Local vLLM daemon (localhost:8000)
        if let Ok(res) = client.get("http://localhost:8000/v1/models").send().await
            && res.status().is_success()
            && let Ok(json) = res.json::<serde_json::Value>().await
            && let Some(arr) = json.get("data").and_then(|d| d.as_array())
        {
            for item in arr {
                if let Some(id_str) = item.get("id").and_then(|n| n.as_str()) {
                    let id = format!("vllm/{}", id_str);
                    if !models.iter().any(|m| m.id == id) {
                        models.push(ModelInfo::new(
                            &id,
                            "vLLM (Local Free)",
                            32_000,
                            8_192,
                            id_str.contains("r1"),
                            false,
                            &format!("Local vLLM model: {}", id_str),
                        ));
                    }
                }
            }
        }

        // 6. Query Groq API if key configured
        if let Some(groq_key) = crate::AuthResolver::resolve_key("groq")
            && let Ok(res) = client
                .get("https://api.groq.com/openai/v1/models")
                .bearer_auth(groq_key)
                .send()
                .await
            && res.status().is_success()
            && let Ok(json) = res.json::<serde_json::Value>().await
            && let Some(arr) = json.get("data").and_then(|d| d.as_array())
        {
            for item in arr {
                if let Some(raw_id) = item.get("id").and_then(|n| n.as_str()) {
                    let id = format!("groq/{}", raw_id);
                    if !models.iter().any(|m| m.id == id) {
                        models.push(ModelInfo::new(
                            &id,
                            "Groq",
                            128_000,
                            8_192,
                            raw_id.contains("r1"),
                            false,
                            &format!("Groq high-speed LPU: {}", raw_id),
                        ));
                    }
                }
            }
        }

        // 7. Query DeepSeek API if key configured
        if let Some(deepseek_key) = crate::AuthResolver::resolve_key("deepseek")
            && let Ok(res) = client
                .get("https://api.deepseek.com/v1/models")
                .bearer_auth(deepseek_key)
                .send()
                .await
            && res.status().is_success()
            && let Ok(json) = res.json::<serde_json::Value>().await
            && let Some(arr) = json.get("data").and_then(|d| d.as_array())
        {
            for item in arr {
                if let Some(raw_id) = item.get("id").and_then(|n| n.as_str()) {
                    let id = format!("deepseek/{}", raw_id);
                    if !models.iter().any(|m| m.id == id) {
                        models.push(ModelInfo::new(
                            &id,
                            "DeepSeek",
                            64_000,
                            8_000,
                            raw_id.contains("reasoner"),
                            false,
                            &format!("DeepSeek API model: {}", raw_id),
                        ));
                    }
                }
            }
        }

        // 8. Query Mistral API if key configured
        if let Some(mistral_key) = crate::AuthResolver::resolve_key("mistral")
            && let Ok(res) = client
                .get("https://api.mistral.ai/v1/models")
                .bearer_auth(mistral_key)
                .send()
                .await
            && res.status().is_success()
            && let Ok(json) = res.json::<serde_json::Value>().await
            && let Some(arr) = json.get("data").and_then(|d| d.as_array())
        {
            for item in arr {
                if let Some(raw_id) = item.get("id").and_then(|n| n.as_str()) {
                    let id = format!("mistral/{}", raw_id);
                    if !models.iter().any(|m| m.id == id) {
                        models.push(ModelInfo::new(
                            &id,
                            "Mistral",
                            128_000,
                            8_192,
                            false,
                            false,
                            &format!("Mistral API model: {}", raw_id),
                        ));
                    }
                }
            }
        }

        // 9. Query OpenCode Zen API if key configured
        if let Some(opencode_key) = crate::AuthResolver::resolve_key("opencode")
            && let Ok(res) = client
                .get("https://opencode.ai/zen/v1/models")
                .bearer_auth(opencode_key)
                .send()
                .await
            && res.status().is_success()
            && let Ok(json) = res.json::<serde_json::Value>().await
            && let Some(arr) = json.get("data").and_then(|d| d.as_array())
        {
            for item in arr {
                if let Some(raw_id) = item.get("id").and_then(|n| n.as_str()) {
                    let id = format!("opencode/{}", raw_id);
                    if !models.iter().any(|m| m.id == id) {
                        models.push(ModelInfo::new(
                            &id,
                            "OpenCode Zen",
                            128_000,
                            16_384,
                            raw_id.contains("architect") || raw_id.contains("reason"),
                            false,
                            &format!("OpenCode Zen: {}", raw_id),
                        ));
                    }
                }
            }
        }

        // 10. Query Kilo Gateway API if key configured
        if let Some(kilo_key) = crate::AuthResolver::resolve_key("kilo")
            && let Ok(res) = client
                .get("https://api.kilo.ai/api/gateway/models")
                .bearer_auth(kilo_key)
                .send()
                .await
            && res.status().is_success()
            && let Ok(json) = res.json::<serde_json::Value>().await
            && let Some(arr) = json.get("data").and_then(|d| d.as_array())
        {
            for item in arr {
                if let Some(raw_id) = item.get("id").and_then(|n| n.as_str()) {
                    let id = format!("kilo/{}", raw_id);
                    if !models.iter().any(|m| m.id == id) {
                        models.push(ModelInfo::new(
                            &id,
                            "Kilo",
                            128_000,
                            16_384,
                            raw_id.contains("r1") || raw_id.contains("o1") || raw_id.contains("o3"),
                            raw_id.contains("sonnet") || raw_id.contains("4o"),
                            &format!("Kilo Gateway: {}", raw_id),
                        ));
                    }
                }
            }
        }

        // 11. Query Agnes Multi-Agent API if key configured
        if let Some(agnes_key) = crate::AuthResolver::resolve_key("agnes")
            && let Ok(res) = client
                .get("https://api.agnes.ai/v1/models")
                .bearer_auth(agnes_key)
                .send()
                .await
            && res.status().is_success()
            && let Ok(json) = res.json::<serde_json::Value>().await
            && let Some(arr) = json.get("data").and_then(|d| d.as_array())
        {
            for item in arr {
                if let Some(raw_id) = item.get("id").and_then(|n| n.as_str()) {
                    let id = format!("agnes/{}", raw_id);
                    if !models.iter().any(|m| m.id == id) {
                        models.push(ModelInfo::new(
                            &id,
                            "Agnes",
                            128_000,
                            16_384,
                            raw_id.contains("architect") || raw_id.contains("deepseek"),
                            raw_id.contains("vision") || raw_id.contains("claude"),
                            &format!("Agnes Gateway: {}", raw_id),
                        ));
                    }
                }
            }
        }

        // Save refreshed catalog to cache
        let catalog = ModelCatalogData {
            last_refreshed: chrono::Utc::now().to_rfc3339(),
            models: models.clone(),
        };
        if let Ok(json) = serde_json::to_string_pretty(&catalog) {
            let _ = fs::write(path, json);
        }

        models
    }

    /// Search/filter model catalog with substring/fuzzy matching
    pub fn search_models<'a>(models: &'a [ModelInfo], query: &str) -> Vec<&'a ModelInfo> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return models.iter().collect();
        }

        let words: Vec<&str> = q.split_whitespace().collect();

        models
            .iter()
            .filter(|m| {
                let text = format!(
                    "{} {} {} {}",
                    m.id.to_lowercase(),
                    m.name.to_lowercase(),
                    m.provider.to_lowercase(),
                    m.description.to_lowercase()
                );
                words.iter().all(|w| text.contains(w))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_static_frontier_models() {
        let models = ModelCatalogLoader::static_frontier_models();
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.id.contains("claude-3-7-sonnet")));
        assert!(models.iter().any(|m| m.id.contains("gpt-4o")));
        assert!(models.iter().any(|m| m.id.contains("deepseek-reasoner")));
        assert!(models.iter().any(|m| m.id.contains("bedrock")));
        assert!(models.iter().any(|m| m.id.contains("copilot")));
        assert!(models.iter().any(|m| m.id.contains("mistral")));
    }

    #[test]
    fn test_search_models() {
        let models = ModelCatalogLoader::static_frontier_models();
        let search_claude = ModelCatalogLoader::search_models(&models, "claude");
        assert!(!search_claude.is_empty());
        assert!(search_claude.iter().all(|m| m.id.contains("claude") || m.provider.to_lowercase().contains("claude")));

        let search_reasoner = ModelCatalogLoader::search_models(&models, "reasoner");
        assert!(!search_reasoner.is_empty());
    }

    #[test]
    fn test_search_models_multi_word() {
        let models = ModelCatalogLoader::static_frontier_models();
        let results = ModelCatalogLoader::search_models(&models, "claude sonnet");
        assert!(!results.is_empty());
        assert!(results.iter().all(|m| m.id.contains("claude") && m.id.contains("sonnet")));
    }
}
