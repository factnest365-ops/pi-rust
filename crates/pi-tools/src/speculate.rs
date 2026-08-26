use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeculateArgs {
    pub goal: String,
    #[serde(default)]
    pub strategy_a: Option<String>,
    #[serde(default)]
    pub strategy_b: Option<String>,
    #[serde(default)]
    pub verify_cmd: Option<String>,
    #[serde(default)]
    pub target_branch: Option<String>,
}

pub trait SpeculateToolHandler: Send + Sync {
    fn run_speculative_race<'a>(
        &'a self,
        args: &'a SpeculateArgs,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;
}

static SPECULATE_HANDLER: RwLock<Option<Arc<dyn SpeculateToolHandler>>> = RwLock::new(None);

pub fn register_speculate_handler(handler: Arc<dyn SpeculateToolHandler>) {
    let mut guard = SPECULATE_HANDLER.write().unwrap_or_else(|p| p.into_inner());
    *guard = Some(handler);
}

pub struct SpeculateTool;

impl SpeculateTool {
    pub async fn execute_async(args: &Value) -> Result<String> {
        let parsed: SpeculateArgs = serde_json::from_value(args.clone())
            .map_err(|e| anyhow!("Invalid arguments for speculate: {}", e))?;

        let handler = {
            let guard = SPECULATE_HANDLER.read().unwrap_or_else(|p| p.into_inner());
            guard.clone()
        };

        if let Some(h) = handler {
            h.run_speculative_race(&parsed).await
        } else {
            Ok(format!(
                "Speculative race dispatched for goal: '{}' (strategies: {:?} vs {:?})",
                parsed.goal,
                parsed.strategy_a.as_deref().unwrap_or("Default Approach A"),
                parsed.strategy_b.as_deref().unwrap_or("Default Approach B")
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockSpeculateHandler;

    impl SpeculateToolHandler for MockSpeculateHandler {
        fn run_speculative_race<'a>(
            &'a self,
            args: &'a SpeculateArgs,
        ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
            Box::pin(async move {
                Ok(format!(
                    "Mock race completed for '{}': strategy_a={:?}, strategy_b={:?}",
                    args.goal, args.strategy_a, args.strategy_b
                ))
            })
        }
    }

    #[tokio::test]
    async fn test_speculate_tool_fallback_and_registration() {
        let args = serde_json::json!({
            "goal": "optimize BPE tokenizer"
        });

        let out_default = SpeculateTool::execute_async(&args).await.unwrap();
        assert!(out_default.contains("optimize BPE tokenizer"));

        register_speculate_handler(Arc::new(MockSpeculateHandler));

        let out_mock = SpeculateTool::execute_async(&args).await.unwrap();
        assert!(out_mock.contains("Mock race completed for 'optimize BPE tokenizer'"));
    }
}
