use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvokeSubagentArgs {
    pub name: String,
    pub task: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub tools: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManageSubagentsArgs {
    pub action: String,
    #[serde(default)]
    pub id: Option<String>,
}

pub trait SubagentToolHandler: Send + Sync {
    fn invoke<'a>(
        &'a self,
        args: &'a InvokeSubagentArgs,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;

    fn manage<'a>(
        &'a self,
        args: &'a ManageSubagentsArgs,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;
}

static SUBAGENT_HANDLER: RwLock<Option<Arc<dyn SubagentToolHandler>>> = RwLock::new(None);

pub fn register_subagent_handler(handler: Arc<dyn SubagentToolHandler>) {
    let mut guard = SUBAGENT_HANDLER.write().unwrap_or_else(|p| p.into_inner());
    *guard = Some(handler);
}

pub struct SubagentTools;

impl SubagentTools {
    pub async fn execute_invoke_async(args: &Value) -> Result<String> {
        let parsed: InvokeSubagentArgs = serde_json::from_value(args.clone())
            .map_err(|e| anyhow!("Invalid arguments for invoke_subagent: {}", e))?;

        let handler = {
            let guard = SUBAGENT_HANDLER.read().unwrap_or_else(|p| p.into_inner());
            guard.clone()
        };

        if let Some(h) = handler {
            h.invoke(&parsed).await
        } else {
            // Default fallback if no runner is registered (e.g. standalone test or uninitialized)
            Ok(format!(
                "Subagent [{}] invoked for task: {} (Handler not registered)",
                parsed.name, parsed.task
            ))
        }
    }

    pub async fn execute_manage_async(args: &Value) -> Result<String> {
        let parsed: ManageSubagentsArgs = serde_json::from_value(args.clone())
            .map_err(|e| anyhow!("Invalid arguments for manage_subagents: {}", e))?;

        let handler = {
            let guard = SUBAGENT_HANDLER.read().unwrap_or_else(|p| p.into_inner());
            guard.clone()
        };

        if let Some(h) = handler {
            h.manage(&parsed).await
        } else {
            match parsed.action.as_str() {
                "list" => Ok("[]".to_string()),
                "status" => {
                    let id = parsed.id.unwrap_or_else(|| "unknown".to_string());
                    Ok(format!("Subagent {} status: Idle", id))
                }
                "kill" => {
                    let id = parsed.id.unwrap_or_else(|| "unknown".to_string());
                    Ok(format!("Subagent {} cancelled", id))
                }
                other => Err(anyhow!("Unknown manage_subagents action: {}", other)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockHandler;
    impl SubagentToolHandler for MockHandler {
        fn invoke<'a>(
            &'a self,
            args: &'a InvokeSubagentArgs,
        ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
            Box::pin(async move {
                Ok(format!(
                    "Mock executed subagent {} for task: {}",
                    args.name, args.task
                ))
            })
        }

        fn manage<'a>(
            &'a self,
            args: &'a ManageSubagentsArgs,
        ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
            Box::pin(async move {
                match args.action.as_str() {
                    "list" => Ok("Mock list: 1 subagent".to_string()),
                    "status" => Ok("Mock status: Running".to_string()),
                    "kill" => Ok(format!(
                        "Mock killed: {}",
                        args.id.as_deref().unwrap_or("none")
                    )),
                    _ => Err(anyhow!("Invalid action")),
                }
            })
        }
    }

    #[tokio::test]
    async fn test_subagent_tool_fallback_and_registration() {
        let invoke_json = serde_json::json!({
            "name": "Scout",
            "task": "Explore codebase"
        });
        let res = SubagentTools::execute_invoke_async(&invoke_json)
            .await
            .unwrap();
        assert!(res.contains("Scout"));

        register_subagent_handler(Arc::new(MockHandler));

        let res_mock = SubagentTools::execute_invoke_async(&invoke_json)
            .await
            .unwrap();
        assert!(res_mock.contains("Mock executed subagent Scout"));

        let manage_json = serde_json::json!({
            "action": "list"
        });
        let res_list = SubagentTools::execute_manage_async(&manage_json)
            .await
            .unwrap();
        assert!(res_list.contains("Mock list"));
    }
}
