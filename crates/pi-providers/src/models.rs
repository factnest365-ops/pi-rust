use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;
use crate::{LMSTUDIO_DEFAULT_HOST, LLAMACPP_DEFAULT_HOST, OLLAMA_API_TAGS, OLLAMA_DEFAULT_HOST, OLLAMA_V1_PATH, VLLM_DEFAULT_HOST};

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

    /// Format context window tokens into human readable string (e.g. 2M, 1M, 200k, 128k, 64k, 32k)
    pub fn format_context_k(context_window: usize) -> String {
        if context_window >= 1_000_000 {
            let m = context_window / 1_000_000;
            let remainder = (context_window % 1_000_000) / 100_000;
            if remainder > 0 {
                format!("{}.{}M", m, remainder)
            } else {
                format!("{}M", m)
            }
        } else if context_window >= 1_000 {
            format!("{}k", context_window / 1_000)
        } else {
            format!("{}", context_window)
        }
    }

    /// Dynamically infers the exact context window and maximum output limits based on model architecture
    pub fn infer_model_limits(model_id: &str, provider: &str) -> (usize, usize) {
        let clean_id = model_id.to_lowercase();
        let clean_prov = provider.to_lowercase();

        // 1. First check high-precision heuristics by known model families
        if clean_id.contains("gemini-3.1")
            || clean_id.contains("gemini-3.0")
            || clean_id.contains("gemini-2.5-pro")
            || clean_id.contains("gemini-2.0-pro")
            || clean_id.contains("gemini-1.5-pro")
            || clean_id.contains("gemini-exp")
        {
            return (2_097_152, 64_000); // 2M Context + 64k Output
        } else if clean_id.contains("gemini-2.5-flash")
            || clean_id.contains("gemini-2.0-flash")
            || clean_id.contains("gemini-flash")
            || clean_id.contains("gemini")
        {
            return (1_048_576, 64_000); // 1M Context + 64k Output
        } else if clean_id.contains("grok-4.6") || clean_id.contains("grok-4") {
            return (1_048_576, 128_000); // 1M Context + 128k Output
        } else if clean_id.contains("opus-5") || clean_id.contains("claude-4") || clean_id.contains("gpt-5") {
            return (500_000, 128_000); // 500k Context + 128k Output
        } else if clean_id.contains("glm-5") || clean_id.contains("kimi-k3") {
            return (256_000, 64_000); // 256k Context
        } else if clean_id.contains("codestral") {
            return (256_000, 16_384); // 256k Context
        } else if clean_id.contains("qwen-3") || clean_id.contains("qwq-72b") {
            return (256_000, 32_768); // 256k Context
        } else if clean_id.contains("claude-3-7") || clean_id.contains("3-7-sonnet") {
            return (200_000, 64_000); // 200k Context + 64k Output
        } else if clean_id.contains("claude-3-5") || clean_id.contains("claude-3") || clean_id.contains("anthropic") {
            return (200_000, 8_192); // 200k Context
        } else if clean_id.contains("o3") || clean_id.contains("o1") || clean_id.contains("grok-3") {
            return (200_000, 100_000); // 200k Context + 100k Output
        } else if clean_id.contains("deepseek-r2") || clean_id.contains("deepseek-v4") {
            return (128_000, 16_384); // 128k Context
        } else if clean_id.contains("deepseek-r1")
            || clean_id.contains("deepseek-reasoner")
            || clean_id.contains("deepseek-chat")
            || clean_id.contains("deepseek-v3")
        {
            return (64_000, 8_000); // 64k Context
        } else if clean_id.contains("grok-2")
            || clean_id.contains("grok")
            || clean_id.contains("qwq")
            || clean_id.contains("llama-4")
            || clean_id.contains("llama-3.3")
            || clean_id.contains("llama-3.1")
            || clean_id.contains("qwen-2.5")
            || clean_id.contains("gpt-4.5")
            || clean_id.contains("gpt-4o")
            || clean_id.contains("gpt-4")
        {
            return (128_000, 16_384); // 128k Context
        } else if clean_id.contains("mistral-small") {
            return (32_000, 4_096); // 32k Context
        }

        // 2. Exact match in static catalog or cached catalog
        let full_prefixed = format!("{}/{}", provider, model_id);
        for m in Self::load_cached_or_static() {
            if m.id.eq_ignore_ascii_case(model_id) || m.id.eq_ignore_ascii_case(&full_prefixed) {
                return (m.context_window, m.max_output);
            }
        }

        // 3. Fallbacks by provider type
        if clean_prov == "ollama" || clean_prov == "llamacpp" {
            (32_000, 4_096)
        } else {
            (128_000, 8_192)
        }
    }

    /// Curated list of all latest frontier, gateway, cloud, and specialized models
    pub fn static_frontier_models() -> Vec<ModelInfo> {
        vec![
            // xAI Grok (2026 SOTA Frontier Flagship)
            ModelInfo::new("xai/grok-4.6", "xAI", 1_048_576, 128_000, true, true, "Flagship Grok 4.6 frontier deep reasoning model with 1M context"),
            ModelInfo::new("xai/grok-4", "xAI", 1_048_576, 128_000, true, true, "xAI Grok 4 frontier multimodal reasoning engine"),
            ModelInfo::new("xai/grok-4-mini", "xAI", 500_000, 64_000, true, false, "xAI Grok 4 Mini ultra-fast reasoning model"),
            ModelInfo::new("xai/grok-3-latest", "xAI", 200_000, 100_000, true, true, "xAI Grok 3 high-capacity reasoning model"),
            ModelInfo::new("openrouter/x-ai/grok-4.6", "OpenRouter", 1_048_576, 128_000, true, true, "Grok 4.6 via OpenRouter Gateway"),

            // Anthropic Claude (2026 Hybrid Reasoning & Frontier Coding)
            ModelInfo::new("anthropic/claude-opus-5", "Anthropic", 500_000, 128_000, true, true, "Claude Opus 5 top-tier deep refactoring & verified agentic coding"),
            ModelInfo::new("anthropic/claude-4-sonnet", "Anthropic", 500_000, 128_000, true, true, "Anthropic Claude 4 Sonnet flagship hybrid reasoning & coding"),
            ModelInfo::new("anthropic/claude-4-opus", "Anthropic", 500_000, 128_000, true, true, "Claude 4 Opus deep architecture analysis & verified coding"),
            ModelInfo::new("anthropic/claude-3-7-sonnet-latest", "Anthropic", 200_000, 64_000, true, true, "Claude 3.7 Sonnet hybrid reasoning & coding"),
            ModelInfo::new("anthropic/claude-3-5-sonnet-latest", "Anthropic", 200_000, 8_192, false, true, "Claude 3.5 Sonnet benchmark coding model"),
            ModelInfo::new("anthropic/claude-3-5-haiku-latest", "Anthropic", 200_000, 8_192, false, false, "Ultra-fast low-latency code assistant"),

            // OpenAI Frontier (2026 Flagship Intelligence & Deep Reasoning)
            ModelInfo::new("openai/gpt-5.6-sol", "OpenAI", 500_000, 128_000, true, true, "GPT-5.6 Sol terminal-first autonomous agent leader"),
            ModelInfo::new("openai/gpt-5-preview", "OpenAI", 500_000, 128_000, true, true, "OpenAI GPT-5 flagship frontier reasoning model"),
            ModelInfo::new("openai/gpt-5", "OpenAI", 500_000, 128_000, true, true, "OpenAI GPT-5 next-generation general intelligence"),
            ModelInfo::new("openai/o3", "OpenAI", 200_000, 100_000, true, true, "OpenAI o3 flagship deep reasoning & verification"),
            ModelInfo::new("openai/o3-mini", "OpenAI", 200_000, 100_000, true, false, "OpenAI o3-mini fast reasoning and code synthesis"),
            ModelInfo::new("openai/gpt-4.5-preview", "OpenAI", 128_000, 16_384, false, true, "GPT-4.5 massive world-knowledge model"),
            ModelInfo::new("openai/gpt-4o", "OpenAI", 128_000, 16_384, false, true, "Flagship versatile multimodal intelligence"),

            // Google Gemini (2026 Multimodal & Ultra-Long 2M Context)
            ModelInfo::new("gemini/gemini-3.1-pro", "Google", 2_097_152, 64_000, true, true, "Gemini 3.1 Pro 2M context large-repo and UI coding leader"),
            ModelInfo::new("gemini/gemini-3.0-pro", "Google", 2_097_152, 64_000, true, true, "Gemini 3.0 Pro frontier 2M context deep reasoning"),
            ModelInfo::new("gemini/gemini-2.5-pro", "Google", 2_097_152, 64_000, true, true, "Gemini 2.5 Pro with 2M context comprehension"),
            ModelInfo::new("gemini/gemini-2.5-flash", "Google", 1_048_576, 64_000, true, true, "Gemini 2.5 Flash low-latency multimodal intelligence"),
            ModelInfo::new("gemini/gemini-2.0-flash", "Google", 1_048_576, 8_192, false, true, "Next-gen multimodal flash speed with 1M context"),
            ModelInfo::new("gemini/gemini-2.0-flash-thinking-exp", "Google (Free Exp)", 1_048_576, 64_000, true, true, "Gemini 2.0 Flash Thinking Experimental reasoning"),

            // DeepSeek & Open Weight Frontier (2026)
            ModelInfo::new("deepseek/deepseek-v4-pro", "DeepSeek", 128_000, 16_384, true, false, "DeepSeek-V4 Pro high-value frontier reasoning"),
            ModelInfo::new("deepseek/deepseek-v4", "DeepSeek", 128_000, 16_384, false, false, "DeepSeek-V4 frontier MoE model"),
            ModelInfo::new("deepseek/deepseek-r2", "DeepSeek", 128_000, 16_384, true, false, "DeepSeek-R2 next-gen open reasoning architecture"),
            ModelInfo::new("deepseek/deepseek-reasoner", "DeepSeek", 64_000, 8_000, true, false, "DeepSeek-R1 open reasoning model"),
            ModelInfo::new("deepseek/deepseek-chat", "DeepSeek", 64_000, 8_000, false, false, "DeepSeek-V3 671B MoE coding model"),

            // GLM & Kimi (2026 Long-Horizon & Frontend SOTA)
            ModelInfo::new("zhipu/glm-5.2", "Zhipu AI", 256_000, 64_000, true, false, "GLM 5.2 premier open-weight long-horizon agentic coding"),
            ModelInfo::new("moonshot/kimi-k3", "Moonshot", 256_000, 64_000, true, true, "Kimi K3 exceptional frontend & web coding model"),

            // Mistral & Codestral (2026 SOTA)
            ModelInfo::new("mistral/codestral-2601", "Mistral", 256_000, 16_384, false, false, "Codestral 2601 frontier coding model with 256k context"),
            ModelInfo::new("mistral/codestral-latest", "Mistral", 256_000, 8_192, false, false, "Specialized coding LLM with 256k context"),
            ModelInfo::new("mistral/mistral-large-latest", "Mistral", 128_000, 8_192, false, false, "Flagship European multilingual model"),

            // Alibaba Qwen & QwQ Reasoning
            ModelInfo::new("qwen/qwen-3.6-coder", "Qwen", 256_000, 32_768, false, false, "Qwen 3.6 Coder open champion for multi-file generation"),
            ModelInfo::new("qwen/qwen-3-coder-32b", "Qwen", 256_000, 32_768, false, false, "Qwen 3 Coder 32B next-gen open coding champion"),
            ModelInfo::new("qwen/qwq-72b", "Qwen", 256_000, 32_768, true, false, "QwQ 72B open frontier reasoning model"),
            ModelInfo::new("qwen/qwq-32b", "Qwen", 128_000, 16_384, true, false, "QwQ 32B open mathematical and code reasoning"),

            // OpenRouter Free Tier Models (Zero Cost)
            ModelInfo::new("openrouter/deepseek/deepseek-r1:free", "OpenRouter (Free)", 64_000, 8_000, true, false, "Free DeepSeek-R1 full open reasoning model"),
            ModelInfo::new("openrouter/deepseek/deepseek-chat:free", "OpenRouter (Free)", 64_000, 8_000, false, false, "Free DeepSeek-V3 671B MoE model"),
            ModelInfo::new("openrouter/meta-llama/llama-3.3-70b-instruct:free", "OpenRouter (Free)", 128_000, 8_192, false, false, "Free Llama 3.3 70B flagship open model"),
            ModelInfo::new("openrouter/google/gemini-2.0-flash-exp:free", "OpenRouter (Free)", 1_048_576, 8_192, false, true, "Free Gemini 2.0 Flash Experimental with 1M context"),
            ModelInfo::new("openrouter/google/gemini-2.0-flash-thinking-exp:free", "OpenRouter (Free)", 1_048_576, 8_192, true, true, "Free Gemini 2.0 Flash Thinking Experimental reasoning"),
            ModelInfo::new("openrouter/qwen/qwen-2.5-coder-32b-instruct:free", "OpenRouter (Free)", 128_000, 8_192, false, false, "Free Qwen 2.5 Coder 32B specialized coding model"),
            ModelInfo::new("openrouter/mistralai/mistral-small-24b-instruct-2501:free", "OpenRouter (Free)", 32_000, 8_192, false, false, "Free Mistral Small 24B lightweight model"),

            // Ollama / Local Models (Zero Cost / Free Local Daemons)
            ModelInfo::new("ollama/qwen2.5-coder:32b", "Ollama (Local Free)", 128_000, 16_384, false, false, "Local Qwen 2.5 Coder 32B running via Ollama"),
            ModelInfo::new("ollama/deepseek-r1:32b", "Ollama (Local Free)", 64_000, 8_000, true, false, "Local DeepSeek R1 32B reasoning running via Ollama"),
            ModelInfo::new("ollama/llama3.3:70b", "Ollama (Local Free)", 128_000, 8_192, false, false, "Local Llama 3.3 70B running via Ollama"),
            ModelInfo::new("lmstudio/local-model", "LM Studio (Local Free)", 128_000, 8_192, false, true, "Local LLM loaded in LM Studio on port 1234"),

            // GitHub Copilot Gateway
            ModelInfo::new("copilot/claude-3.5-sonnet", "GitHub Copilot", 200_000, 8_192, false, true, "GitHub Copilot Claude 3.5 Sonnet pipeline"),
            ModelInfo::new("copilot/gpt-4o", "GitHub Copilot", 128_000, 8_192, false, true, "GitHub Copilot GPT-4o pipeline"),
            ModelInfo::new("copilot/o3-mini", "GitHub Copilot", 200_000, 100_000, true, false, "GitHub Copilot o3-mini reasoning"),

            // Amazon Bedrock Gateway
            ModelInfo::new("bedrock/anthropic.claude-3-5-sonnet-20241022-v2:0", "Amazon Bedrock", 200_000, 8_192, false, true, "Amazon Bedrock Claude 3.5 Sonnet"),
            ModelInfo::new("bedrock/anthropic.claude-3-5-haiku-20241022-v1:0", "Amazon Bedrock", 200_000, 8_192, false, false, "Amazon Bedrock Claude 3.5 Haiku"),

            // Groq High-Throughput (300+ tok/s)
            ModelInfo::new("groq/llama-3.3-70b-versatile", "Groq", 128_000, 32_768, false, false, "Llama 3.3 70B running at 300+ tok/s"),
            ModelInfo::new("groq/deepseek-r1-distill-llama-70b", "Groq", 128_000, 8_192, true, false, "High-speed DeepSeek-R1 distillation"),

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

        // 3. Query Local Ollama daemon via constant
        if let Ok(res) = client.get(format!("{}{}", OLLAMA_DEFAULT_HOST, OLLAMA_API_TAGS)).send().await
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

        // 3. Query Local LM Studio daemon
        if let Ok(res) = client.get(format!("{}{}{}", LMSTUDIO_DEFAULT_HOST, OLLAMA_V1_PATH, "/models")).send().await
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

        // 4. Query Local llama.cpp daemon
        if let Ok(res) = client.get(format!("{}{}{}", LLAMACPP_DEFAULT_HOST, OLLAMA_V1_PATH, "/models")).send().await
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

        // 5. Query Local vLLM daemon
        if let Ok(res) = client.get(format!("{}{}{}", VLLM_DEFAULT_HOST, OLLAMA_V1_PATH, "/models")).send().await
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

    #[test]
    fn test_search_models_empty_and_no_match() {
        let models = ModelCatalogLoader::static_frontier_models();
        let all_models = ModelCatalogLoader::search_models(&models, "   ");
        assert_eq!(all_models.len(), models.len());

        let no_match = ModelCatalogLoader::search_models(&models, "nonexistent_model_xyz_12345");
        assert!(no_match.is_empty());
    }

    #[test]
    fn test_model_info_properties() {
        let m = ModelInfo::new("my-prov/custom-llm", "MyProvider", 65536, 4096, true, true, "Custom test description");
        assert_eq!(m.id, "my-prov/custom-llm");
        assert_eq!(m.name, "custom-llm");
        assert_eq!(m.provider, "MyProvider");
        assert_eq!(m.context_window, 65536);
        assert_eq!(m.max_output, 4096);
        assert!(m.supports_reasoning);
        assert!(m.supports_vision);
    }

    #[test]
    fn test_custom_models_config_deserialization() {
        let json_str = r#"{
            "providers": {
                "custom_gw": {
                    "baseUrl": "https://custom.ai/v1",
                    "apiKey": "gw_key",
                    "models": [
                        {
                            "id": "custom-fast",
                            "name": "Custom Fast",
                            "contextWindow": 32000,
                            "reasoning": true
                        }
                    ]
                }
            },
            "models": [
                {
                    "id": "standalone/coder-v1",
                    "provider": "Standalone",
                    "contextWindow": 64000
                }
            ]
        }"#;

        let cfg: CustomModelsConfig = serde_json::from_str(json_str).expect("Valid JSON config");
        assert!(cfg.providers.is_some());
        let provs = cfg.providers.unwrap();
        assert!(provs.contains_key("custom_gw"));
        assert_eq!(provs["custom_gw"].base_url.as_deref(), Some("https://custom.ai/v1"));
        let p_models = provs["custom_gw"].models.as_ref().unwrap();
        assert_eq!(p_models[0].id, "custom-fast");
        assert_eq!(p_models[0].context_window, Some(32000));
        assert_eq!(p_models[0].reasoning, Some(true));

        let s_models = cfg.models.unwrap();
        assert_eq!(s_models[0].id, "standalone/coder-v1");
        assert_eq!(s_models[0].provider.as_deref(), Some("Standalone"));
    }

    #[test]
    fn test_infer_model_limits_dynamic_resolution() {
        // xAI Grok 4.6 (1M context + 128k output)
        let (cw_grok, out_grok) = ModelCatalogLoader::infer_model_limits("xai/grok-4.6", "xai");
        assert_eq!(cw_grok, 1_048_576);
        assert_eq!(out_grok, 128_000);

        // OpenAI GPT-5 (500k context + 128k output)
        let (cw_gpt5, out_gpt5) = ModelCatalogLoader::infer_model_limits("openai/gpt-5-preview", "openai");
        assert_eq!(cw_gpt5, 500_000);
        assert_eq!(out_gpt5, 128_000);

        // Anthropic Claude 4 (500k context + 128k output)
        let (cw_claude4, out_claude4) = ModelCatalogLoader::infer_model_limits("anthropic/claude-4-sonnet", "anthropic");
        assert_eq!(cw_claude4, 500_000);
        assert_eq!(out_claude4, 128_000);

        // Gemini 3.0 2M
        let (cw_gemini_pro, _) = ModelCatalogLoader::infer_model_limits("gemini/gemini-3.0-pro", "gemini");
        assert_eq!(cw_gemini_pro, 2_097_152);

        // Gemini 2.0 Flash 1M
        let (cw_gemini_flash, _) = ModelCatalogLoader::infer_model_limits("gemini-2.0-flash", "gemini");
        assert_eq!(cw_gemini_flash, 1_048_576);

        // Codestral 256k
        let (cw_codestral, _) = ModelCatalogLoader::infer_model_limits("mistral/codestral-2601", "mistral");
        assert_eq!(cw_codestral, 256_000);

        // Claude 3.7 200k
        let (cw_claude, max_out) = ModelCatalogLoader::infer_model_limits("anthropic/claude-3-7-sonnet-latest", "anthropic");
        assert_eq!(cw_claude, 200_000);
        assert_eq!(max_out, 64_000);

        // OpenAI o1 / o3-mini 200k
        let (cw_o1, _) = ModelCatalogLoader::infer_model_limits("openai/o1", "openai");
        assert_eq!(cw_o1, 200_000);

        // DeepSeek 64k
        let (cw_deepseek, _) = ModelCatalogLoader::infer_model_limits("deepseek/deepseek-reasoner", "deepseek");
        assert_eq!(cw_deepseek, 64_000);

        // Local Ollama 32k
        let (cw_ollama, _) = ModelCatalogLoader::infer_model_limits("mistral-small", "ollama");
        assert_eq!(cw_ollama, 32_000);
    }

    #[test]
    fn test_format_context_k_representation() {
        assert_eq!(ModelCatalogLoader::format_context_k(2_097_152), "2M");
        assert_eq!(ModelCatalogLoader::format_context_k(1_048_576), "1M");
        assert_eq!(ModelCatalogLoader::format_context_k(256_000), "256k");
        assert_eq!(ModelCatalogLoader::format_context_k(200_000), "200k");
        assert_eq!(ModelCatalogLoader::format_context_k(128_000), "128k");
        assert_eq!(ModelCatalogLoader::format_context_k(64_000), "64k");
        assert_eq!(ModelCatalogLoader::format_context_k(32_000), "32k");
    }
}
