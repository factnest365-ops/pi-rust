#![allow(unused_imports)]

use crate::ModelConfig;
use crate::ProviderClient;
use crate::ProviderResponse;
use crate::ProviderToolCall;
use serde_json::json;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_best_response_prefers_tool_calls_then_text_length() {
        let candidates = vec![
            ProviderResponse { text: "short".to_string(), tool_calls: vec![] },
            ProviderResponse { text: "much longer answer".to_string(), tool_calls: vec![] },
            ProviderResponse {
                text: "small".to_string(),
                tool_calls: vec![ProviderToolCall { id: "c1".to_string(), name: "read".to_string(), arguments: json!({}) }],
            },
        ];

        let best = ProviderClient::select_best_response(candidates);
        assert_eq!(best.tool_calls.len(), 1);
        assert_eq!(best.tool_calls[0].name, "read");
    }

    #[test]
    fn test_select_best_response_single_candidate() {
        let only = ProviderResponse { text: "only".to_string(), tool_calls: vec![] };
        let best = ProviderClient::select_best_response(vec![only.clone()]);
        assert_eq!(best.text, "only");
    }

    #[test]
    fn test_model_config_best_of_n_parsing() {
        let json = r#"{"provider":"openai","model_id":"m","api_key":"k","best_of_n":4,"context_window":100}"#;
        let cfg: ModelConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.best_of_n, Some(4));

        let default_json = r#"{"provider":"openai","model_id":"m","api_key":"k","context_window":100}"#;
        let default_cfg: ModelConfig = serde_json::from_str(default_json).unwrap();
        assert_eq!(default_cfg.best_of_n, None);
    }
}
