use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextBudget {
    pub total_limit: usize,
    pub system_tokens: usize,
    pub conversation_tokens: usize,
    pub remaining_tokens: usize,
    pub percent_used: f32,
    pub needs_compaction: bool,
}

pub struct TokenProfiler;

impl TokenProfiler {
    /// Fast BPE / token estimator across model architectures
    pub fn estimate_tokens(text: &str, model_id: &str) -> usize {
        if text.is_empty() {
            return 0;
        }

        // CJK and non-ASCII characters have higher token-to-character ratio
        let mut ascii_count = 0usize;
        let mut non_ascii_count = 0usize;
        let mut whitespace_count = 0usize;

        for ch in text.chars() {
            if ch.is_ascii_whitespace() {
                whitespace_count += 1;
            } else if ch.is_ascii() {
                ascii_count += 1;
            } else {
                non_ascii_count += 1;
            }
        }

        // Factor estimation calibrated for modern code/chat models
        let base_estimate = if model_id.contains("deepseek") || model_id.contains("r1") {
            (ascii_count as f64 / 3.6) + (non_ascii_count as f64 * 1.2) + (whitespace_count as f64 * 0.3)
        } else if model_id.contains("claude") || model_id.contains("anthropic") {
            (ascii_count as f64 / 3.7) + (non_ascii_count as f64 * 1.3) + (whitespace_count as f64 * 0.3)
        } else {
            (ascii_count as f64 / 4.0) + (non_ascii_count as f64 * 1.4) + (whitespace_count as f64 * 0.3)
        };

        base_estimate.ceil() as usize
    }

    /// Calculate context window usage and compaction threshold
    pub fn compute_budget(
        system_prompt: &str,
        conversation_history: &str,
        model_id: &str,
        context_limit: usize,
    ) -> ContextBudget {
        let safe_limit = context_limit.max(1);
        let system_tokens = Self::estimate_tokens(system_prompt, model_id);
        let conversation_tokens = Self::estimate_tokens(conversation_history, model_id);
        let total_used = system_tokens + conversation_tokens;
        let remaining_tokens = context_limit.saturating_sub(total_used);
        let percent_used = ((total_used as f32 / safe_limit as f32) * 100.0).clamp(0.0, 1000.0);
        let needs_compaction = percent_used >= 80.0;

        ContextBudget {
            total_limit: context_limit,
            system_tokens,
            conversation_tokens,
            remaining_tokens,
            percent_used,
            needs_compaction,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_estimation() {
        let sample = "fn main() {\n    println!(\"Hello, world!\");\n}";
        let tokens = TokenProfiler::estimate_tokens(sample, "kilo/deepseek-r1");
        assert!((8..=20).contains(&tokens));
    }

    #[test]
    fn test_budget_computation() {
        let sys = "You are a helpful assistant.";
        let conv = "User: Hello!\nAssistant: Hi!";
        let budget = TokenProfiler::compute_budget(sys, conv, "claude-3-5-sonnet", 128_000);

        assert_eq!(budget.total_limit, 128_000);
        assert!(!budget.needs_compaction);
        assert!(budget.percent_used < 1.0);
    }

    #[test]
    fn test_budget_computation_zero_limit() {
        let sys = "You are a helpful assistant.";
        let conv = "User: Hello!";
        let budget = TokenProfiler::compute_budget(sys, conv, "claude-3-5-sonnet", 0);

        assert_eq!(budget.total_limit, 0);
        assert!(budget.needs_compaction);
        assert_eq!(budget.remaining_tokens, 0);
    }
}
