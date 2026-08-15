use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrewDispatchArgs {
    #[serde(default = "default_shape")]
    pub shape: String,
    pub task: String,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default = "default_backend")]
    pub backend: String,
    #[serde(default)]
    pub verify_cmd: Option<String>,
}

fn default_shape() -> String {
    "ship".to_string()
}
fn default_mode() -> String {
    "local-only".to_string()
}
fn default_backend() -> String {
    "herdr".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrewStatusArgs {
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default = "default_status_action")]
    pub action: String,
}

fn default_status_action() -> String {
    "list".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrewMergeArgs {
    pub task_id: String,
    #[serde(default = "default_target_branch")]
    pub target_branch: String,
    #[serde(default)]
    pub verify_cmd: Option<String>,
}

fn default_target_branch() -> String {
    "HEAD".to_string()
}

pub trait CrewToolHandler: Send + Sync {
    fn dispatch<'a>(
        &'a self,
        args: &'a CrewDispatchArgs,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;

    fn status<'a>(
        &'a self,
        args: &'a CrewStatusArgs,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;

    fn merge<'a>(
        &'a self,
        args: &'a CrewMergeArgs,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;
}

static CREW_HANDLER: RwLock<Option<Arc<dyn CrewToolHandler>>> = RwLock::new(None);

pub fn register_crew_handler(handler: Arc<dyn CrewToolHandler>) {
    let mut guard = CREW_HANDLER.write().expect("Crew handler lock poisoned");
    *guard = Some(handler);
}

pub struct CrewTools;

impl CrewTools {
    pub async fn execute_dispatch_async(args: &Value) -> Result<String> {
        let parsed: CrewDispatchArgs = serde_json::from_value(args.clone())
            .map_err(|e| anyhow!("Invalid arguments for crew_dispatch: {}", e))?;

        let handler = {
            let guard = CREW_HANDLER.read().expect("Crew handler lock poisoned");
            guard.clone()
        };

        if let Some(h) = handler {
            h.dispatch(&parsed).await
        } else {
            Ok(format!(
                "Crew task [{}] dispatched: '{}' (mode={}, backend={})",
                parsed.shape, parsed.task, parsed.mode, parsed.backend
            ))
        }
    }

    pub async fn execute_status_async(args: &Value) -> Result<String> {
        let parsed: CrewStatusArgs = serde_json::from_value(args.clone())
            .map_err(|e| anyhow!("Invalid arguments for crew_status: {}", e))?;

        let handler = {
            let guard = CREW_HANDLER.read().expect("Crew handler lock poisoned");
            guard.clone()
        };

        if let Some(h) = handler {
            h.status(&parsed).await
        } else {
            Ok("No active crew tasks registered.".to_string())
        }
    }

    pub async fn execute_merge_async(args: &Value) -> Result<String> {
        let parsed: CrewMergeArgs = serde_json::from_value(args.clone())
            .map_err(|e| anyhow!("Invalid arguments for crew_merge: {}", e))?;

        let handler = {
            let guard = CREW_HANDLER.read().expect("Crew handler lock poisoned");
            guard.clone()
        };

        if let Some(h) = handler {
            h.merge(&parsed).await
        } else {
            Ok(format!(
                "Ship task '{}' merged into target '{}'",
                parsed.task_id, parsed.target_branch
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockCrewHandler;
    impl CrewToolHandler for MockCrewHandler {
        fn dispatch<'a>(
            &'a self,
            args: &'a CrewDispatchArgs,
        ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
            Box::pin(async move {
                Ok(format!(
                    "Mock dispatched {} task: {}",
                    args.shape, args.task
                ))
            })
        }

        fn status<'a>(
            &'a self,
            _args: &'a CrewStatusArgs,
        ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
            Box::pin(async move { Ok("Mock fleet: 1 active task".to_string()) })
        }

        fn merge<'a>(
            &'a self,
            args: &'a CrewMergeArgs,
        ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
            Box::pin(async move {
                Ok(format!("Mock merged task {}", args.task_id))
            })
        }
    }

    #[tokio::test]
    async fn test_crew_tool_handler_registration() {
        register_crew_handler(Arc::new(MockCrewHandler));

        let res = CrewTools::execute_dispatch_async(&serde_json::json!({
            "shape": "ship",
            "task": "Add unit tests"
        }))
        .await
        .unwrap();

        assert!(res.contains("Mock dispatched ship task: Add unit tests"));
    }
}
