//! Canonical provider registry — single source of truth.
//!
//! UI order is cloud -> gateway -> local (matches historic provider_picker
//! grouping to avoid snapshot churn).

use super::constants::{
    LLAMACPP_DEFAULT_ENDPOINT, LMSTUDIO_DEFAULT_ENDPOINT, OLLAMA_DEFAULT_ENDPOINT,
    VLLM_DEFAULT_ENDPOINT,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderDef {
    pub id: &'static str,
    pub name: &'static str,
    pub endpoint: &'static str,
    pub is_local: bool,
    /// Alternate ids accepted for this provider (excludes [`ProviderDef::id`] itself).
    pub aliases: &'static [&'static str],
}

pub const PROVIDERS: &[ProviderDef] = &[
    ProviderDef {
        id: "anthropic",
        name: "Anthropic Claude",
        endpoint: "https://api.anthropic.com/v1",
        is_local: false,
        aliases: &["claude"],
    },
    ProviderDef {
        id: "openai",
        name: "OpenAI Frontier",
        endpoint: "https://api.openai.com/v1",
        is_local: false,
        aliases: &["gpt"],
    },
    ProviderDef {
        id: "gemini",
        name: "Google Gemini",
        endpoint: "https://generativelanguage.googleapis.com/v1beta/openai",
        is_local: false,
        aliases: &["google"],
    },
    ProviderDef {
        id: "deepseek",
        name: "DeepSeek AI",
        endpoint: "https://api.deepseek.com/v1",
        is_local: false,
        aliases: &[],
    },
    ProviderDef {
        id: "groq",
        name: "Groq LPU Engine",
        endpoint: "https://api.groq.com/openai/v1",
        is_local: false,
        aliases: &[],
    },
    ProviderDef {
        id: "openrouter",
        name: "OpenRouter (250+ Models)",
        endpoint: "https://openrouter.ai/api/v1",
        is_local: false,
        aliases: &[],
    },
    ProviderDef {
        id: "mistral",
        name: "Mistral & Codestral",
        endpoint: "https://api.mistral.ai/v1",
        is_local: false,
        aliases: &["codestral"],
    },
    ProviderDef {
        id: "cerebras",
        name: "Cerebras CS-3 Wafer",
        endpoint: "https://api.cerebras.ai/v1",
        is_local: false,
        aliases: &[],
    },
    ProviderDef {
        id: "copilot",
        name: "GitHub Copilot",
        endpoint: "https://api.githubcopilot.com",
        is_local: false,
        aliases: &["github-copilot"],
    },
    ProviderDef {
        id: "bedrock",
        name: "Amazon Bedrock",
        endpoint: "https://bedrock-runtime.us-east-1.amazonaws.com",
        is_local: false,
        aliases: &["amazon-bedrock"],
    },
    ProviderDef {
        id: "xai",
        name: "xAI Grok",
        endpoint: "https://api.x.ai/v1",
        is_local: false,
        aliases: &[],
    },
    ProviderDef {
        id: "together",
        name: "Together AI",
        endpoint: "https://api.together.xyz/v1",
        is_local: false,
        aliases: &["together-ai"],
    },
    ProviderDef {
        id: "fireworks",
        name: "Fireworks AI",
        endpoint: "https://api.fireworks.ai/inference/v1",
        is_local: false,
        aliases: &[],
    },
    ProviderDef {
        id: "perplexity",
        name: "Perplexity AI",
        endpoint: "https://api.perplexity.ai",
        is_local: false,
        aliases: &[],
    },
    ProviderDef {
        id: "qwen",
        name: "Qwen Token Plan (Alibaba)",
        endpoint: "https://dashscope-intl.aliyuncs.com/compatible-mode/v1",
        is_local: false,
        aliases: &["qwen-token-plan"],
    },
    ProviderDef {
        id: "xiaomi",
        name: "Xiaomi MiMo",
        endpoint: "https://api.mimo.xiaomi.com/v1",
        is_local: false,
        aliases: &["mimo"],
    },
    ProviderDef {
        id: "moonshot",
        name: "Moonshot AI / Kimi",
        endpoint: "https://api.moonshot.cn/v1",
        is_local: false,
        aliases: &["kimi"],
    },
    ProviderDef {
        id: "huggingface",
        name: "Hugging Face Hub",
        endpoint: "https://api-inference.huggingface.co/v1",
        is_local: false,
        aliases: &["hf"],
    },
    ProviderDef {
        id: "opencode",
        name: "OpenCode Zen Gateway",
        endpoint: "https://opencode.ai/zen/v1",
        is_local: false,
        aliases: &["zen", "opencode-zen", "opencode-go"],
    },
    ProviderDef {
        id: "kilo",
        name: "Kilo Gateway",
        endpoint: "https://api.kilo.ai/api/gateway",
        is_local: false,
        aliases: &["kilo-gateway"],
    },
    ProviderDef {
        id: "agnes",
        name: "Agnes Orchestrator",
        endpoint: "https://api.agnes.ai/v1",
        is_local: false,
        aliases: &["agnes-gateway"],
    },
    ProviderDef {
        id: "ollama",
        name: "Ollama (Local :11434)",
        endpoint: OLLAMA_DEFAULT_ENDPOINT,
        is_local: true,
        aliases: &[],
    },
    ProviderDef {
        id: "lmstudio",
        name: "LM Studio (Local :1234)",
        endpoint: LMSTUDIO_DEFAULT_ENDPOINT,
        is_local: true,
        aliases: &[],
    },
    ProviderDef {
        id: "llamacpp",
        name: "llama.cpp (Local :8080)",
        endpoint: LLAMACPP_DEFAULT_ENDPOINT,
        is_local: true,
        aliases: &["llama.cpp"],
    },
    ProviderDef {
        id: "vllm",
        name: "vLLM (Local :8000)",
        endpoint: VLLM_DEFAULT_ENDPOINT,
        is_local: true,
        aliases: &[],
    },
];

/// Provider ids in canonical registry order (cloud -> gateway -> local).
pub const PROVIDER_IDS: &[&str] = &[
    "anthropic",
    "openai",
    "gemini",
    "deepseek",
    "groq",
    "openrouter",
    "mistral",
    "cerebras",
    "copilot",
    "bedrock",
    "xai",
    "together",
    "fireworks",
    "perplexity",
    "qwen",
    "xiaomi",
    "moonshot",
    "huggingface",
    "opencode",
    "kilo",
    "agnes",
    "ollama",
    "lmstudio",
    "llamacpp",
    "vllm",
];

/// Find a provider by canonical id or alias (case-insensitive).
pub fn get(provider: &str) -> Option<&'static ProviderDef> {
    let needle = provider.trim().to_lowercase();
    PROVIDERS.iter().find(|p| {
        p.id == needle || p.aliases.iter().any(|a| *a == needle)
    })
}

/// Default base URL for a provider id or alias. Returns `None` for unknown ids.
pub fn endpoint_for(provider: &str) -> Option<&'static str> {
    get(provider).map(|p| p.endpoint)
}

/// Canonical id for an id-or-alias input (case-insensitive). Returns `None` if unknown.
pub fn canonical_id(provider: &str) -> Option<&'static str> {
    get(provider).map(|p| p.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn registry_has_25_unique_ids_in_order() {
        assert_eq!(PROVIDERS.len(), 25);
        assert_eq!(PROVIDER_IDS.len(), 25);
        let mut seen = HashSet::new();
        for (i, p) in PROVIDERS.iter().enumerate() {
            assert_eq!(p.id, PROVIDER_IDS[i], "PROVIDERS/PROVIDER_IDS order drift at {i}");
            assert!(seen.insert(p.id), "duplicate provider id: {}", p.id);
            assert!(!p.aliases.contains(&p.id), "id listed in own aliases: {}", p.id);
        }
    }

    #[test]
    fn registry_alias_lookup() {
        assert_eq!(endpoint_for("claude"), endpoint_for("anthropic"));
        assert_eq!(endpoint_for("gpt"), endpoint_for("openai"));
        assert_eq!(endpoint_for("google"), endpoint_for("gemini"));
        assert_eq!(endpoint_for("kilo-gateway"), endpoint_for("kilo"));
        assert_eq!(endpoint_for("codestral"), endpoint_for("mistral"));
        assert_eq!(endpoint_for("llama.cpp"), endpoint_for("llamacpp"));
        assert_eq!(canonical_id("KIMI"), Some("moonshot"));
        assert!(endpoint_for("does-not-exist").is_none());
    }

    #[test]
    fn registry_local_flags() {
        let locals: Vec<&&str> = PROVIDER_IDS.iter().filter(|id| get(id).is_some_and(|p| p.is_local)).collect();
        assert_eq!(locals, vec![&"ollama", &"lmstudio", &"llamacpp", &"vllm"]);
    }
}
