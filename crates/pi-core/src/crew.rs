use crate::firstmate::{CrewBackend, CrewMergeMode, CrewTask, CrewTaskShape, FirstMateDistro};
use anyhow::Result;
use chrono::{DateTime, Utc};
use pi_tools::{CrewDispatchArgs, CrewMergeArgs, CrewStatusArgs, CrewToolHandler};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrewTaskSummary {
    pub id: String,
    pub shape: String,
    pub task: String,
    pub status: String,
    pub created_at: String,
    pub finished_at: Option<String>,
    pub worktree_path: Option<String>,
    pub branch_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrewDispatchStats {
    pub task_id: String,
    pub dispatched_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub elapsed_ms: Option<i64>,
    pub status: String,
    pub wins: usize,
    pub visits: usize,
    pub uct: Option<f64>,
    pub cost: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrewDispatchResult {
    pub task: CrewTask,
    pub subcrews: Vec<CrewDispatchArgs>,
    pub stats: CrewDispatchStats,
}

#[derive(Clone)]
pub struct CrewDispatchCore {
    distro: Arc<FirstMateDistro>,
}

impl CrewDispatchCore {
    pub fn new(distro: Arc<FirstMateDistro>) -> Self {
        Self { distro }
    }

    pub async fn dispatch(
        &self,
        task: &str,
        n: usize,
    ) -> Result<Vec<CrewDispatchResult>> {
        let n = Self::clamp_n(n);
        let mut results = Vec::with_capacity(n);

        for index in 0..n {
            let dispatched_at = Utc::now();
            let shape = if index == 0 {
                CrewTaskShape::Ship
            } else {
                CrewTaskShape::Scout
            };
            let created = self
                .distro
                .dispatch(
                    shape,
                    format!("{} [crew {}]", task, index + 1),
                    CrewMergeMode::LocalOnly,
                    CrewBackend::Herdr,
                    None,
                )
                .await?;

            let uct = 1.0 / (1.0 + (index as f64 + 1.0));
            let stats = CrewDispatchStats {
                task_id: created.id.clone(),
                dispatched_at,
                finished_at: None,
                elapsed_ms: None,
                status: format!("{:?}", created.status),
                wins: 0,
                visits: index + 1,
                uct: Some(uct),
                cost: None,
            };

            results.push(CrewDispatchResult {
                task: created,
                subcrews: Vec::new(),
                stats,
            });
        }

        Ok(results)
    }

    pub async fn status(&self) -> Result<Vec<CrewTaskSummary>> {
        let tasks = self.distro.reconcile_fleet().await;
        Ok(tasks
            .into_iter()
            .map(|task| CrewTaskSummary {
                id: task.id,
                shape: format!("{:?}", task.shape),
                task: task.task,
                status: format!("{:?}", task.status),
                created_at: task.created_at,
                finished_at: task.finished_at,
                worktree_path: task.worktree_path.map(|p| p.to_string_lossy().to_string()),
                branch_name: task.branch_name,
            })
            .collect())
    }

    pub async fn merge(&self, task_id: &str, target_branch: &str) -> Result<String> {
        self.distro
            .merge_ship_task(task_id, target_branch, None)
            .await
    }

    pub async fn cancel(&self, task_id: &str) -> Result<()> {
        self.distro.cancel_task(task_id).await
    }

    pub async fn reconcile(&self) -> Result<Vec<CrewTaskSummary>> {
        self.status().await
    }

    fn clamp_n(n: usize) -> usize {
        n.clamp(3, 5)
    }
}

#[derive(Clone)]
pub struct CrewToolHandlerBridge {
    core: Arc<CrewDispatchCore>,
}

impl CrewToolHandlerBridge {
    pub fn new(core: Arc<CrewDispatchCore>) -> Self {
        Self { core }
    }
}

impl CrewToolHandler for CrewToolHandlerBridge {
    fn dispatch<'a>(
        &'a self,
        args: &'a CrewDispatchArgs,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            let n = Self::extract_n(args);
            let results = self.core.dispatch(&args.task, n).await?;
            serde_json::to_string_pretty(&results)
                .map_err(|e| anyhow::anyhow!("Failed to serialize crew dispatch results: {}", e))
        })
    }

    fn status<'a>(
        &'a self,
        args: &'a CrewStatusArgs,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            let action = args.action.to_lowercase();
            if action == "cancel" {
                let task_id = args
                    .task_id
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("Missing 'task_id' for crew cancel"))?;
                self.core.cancel(task_id).await?;
                return Ok(format!("Cancelled crew task {}", task_id));
            }
            let summary = self.core.status().await?;
            serde_json::to_string_pretty(&summary)
                .map_err(|e| anyhow::anyhow!("Failed to serialize crew status: {}", e))
        })
    }

    fn merge<'a>(
        &'a self,
        args: &'a CrewMergeArgs,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            self.core
                .merge(&args.task_id, &args.target_branch)
                .await
        })
    }

    fn cancel<'a>(
        &'a self,
        args: &'a CrewStatusArgs,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            let task_id = args
                .task_id
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("Missing 'task_id' for crew cancel"))?;
            self.core.cancel(task_id).await?;
            Ok(format!("Crew task {} cancelled", task_id))
        })
    }

    fn reconcile<'a>(
        &'a self,
        _args: &'a CrewStatusArgs,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            let summary = self.core.reconcile().await?;
            serde_json::to_string_pretty(&summary)
                .map_err(|e| anyhow::anyhow!("Failed to serialize crew reconcile: {}", e))
        })
    }
}

impl CrewToolHandlerBridge {
    fn extract_n(args: &CrewDispatchArgs) -> usize {
        let mode = args.mode.to_lowercase();
        let backend = args.backend.to_lowercase();
        let task = args.task.to_lowercase();

        if let Some(n) = args.n { return n.clamp(3, 5); }
        if mode == "full" {
            return 5;
        }
        if backend == "herdr" && task.contains("grep") {
            return 4;
        }
        3
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::tempdir;

    fn init_test_repo(dir: &std::path::Path) {
        Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(dir)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Pi Crew"])
            .current_dir(dir)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "crew@pi.dev"])
            .current_dir(dir)
            .output()
            .unwrap();
        std::fs::write(dir.join("README.md"), "# repo\n").unwrap();
        Command::new("git")
            .args(["add", "README.md"])
            .current_dir(dir)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(dir)
            .output()
            .unwrap();
    }

    #[tokio::test]
    async fn test_crew_dispatch_clamps_and_shapes() {
        let tmp = tempdir().unwrap();
        init_test_repo(tmp.path());
        let core = CrewDispatchCore::new(Arc::new(FirstMateDistro::new(tmp.path())));

        let three = core.dispatch("grep test", 3).await.unwrap();
        assert_eq!(three.len(), 3);
        let five = core.dispatch("grep test", 9).await.unwrap();
        assert_eq!(five.len(), 5);

        assert_eq!(three[0].task.shape, CrewTaskShape::Ship);
        assert_eq!(three[1].task.shape, CrewTaskShape::Scout);
    }

    #[tokio::test]
    async fn test_crew_merge_and_status_flow() {
        let tmp = tempdir().unwrap();
        init_test_repo(tmp.path());
        let core = CrewDispatchCore::new(Arc::new(FirstMateDistro::new(tmp.path())));

        let created = core.dispatch("implement feature", 3).await.unwrap();
        let ship_id = &created[0].task.id;

        if let Some(ref wt) = created[0].task.worktree_path {
            std::fs::write(wt.join("feature.txt"), "crew output\n").unwrap();
            Command::new("git")
                .args(["add", "feature.txt"])
                .current_dir(wt)
                .output()
                .unwrap();
            Command::new("git")
                .args(["commit", "-m", "crew feature"])
                .current_dir(wt)
                .output()
                .unwrap();
        }

        let merge = core.merge(ship_id, "HEAD").await.unwrap();
        assert!(merge.contains("Successfully merged") || merge.contains("Merge"));

        let status = core.status().await.unwrap();
        assert!(status.iter().any(|s| s.id == *ship_id));
    }

    #[tokio::test]
    async fn test_crew_cancel_and_reconcile() {
        let tmp = tempdir().unwrap();
        init_test_repo(tmp.path());
        let core = Arc::new(CrewDispatchCore::new(Arc::new(FirstMateDistro::new(tmp.path()))));
        let handler = CrewToolHandlerBridge::new(core.clone());

        let dispatch_args = CrewDispatchArgs {
            shape: "ship".to_string(),
            task: "cancelable crew task".to_string(),
            mode: "local-only".to_string(),
            backend: "herdr".to_string(),
            verify_cmd: None,
            n: Some(3),
        };
        let dispatched = handler.dispatch(&dispatch_args).await.unwrap();
        let results: Vec<CrewDispatchResult> = serde_json::from_str(&dispatched).unwrap();
        let task_id = results[0].task.id.clone();

        let cancel_args = CrewStatusArgs {
            task_id: Some(task_id.clone()),
            action: "cancel".to_string(),
        };
        handler.cancel(&cancel_args).await.unwrap();

        let reconcile = handler.reconcile(&CrewStatusArgs {
            task_id: None,
            action: "reconcile".to_string(),
        }).await.unwrap();
        assert!(reconcile.contains(&task_id));
    }

    #[tokio::test]
    async fn test_crew_handler_bridge_mock_dispatch() {
        let tmp = tempdir().unwrap();
        init_test_repo(tmp.path());
        let core = Arc::new(CrewDispatchCore::new(Arc::new(FirstMateDistro::new(tmp.path()))));
        let handler = CrewToolHandlerBridge::new(core);

        let args = CrewDispatchArgs {
            shape: "ship".to_string(),
            task: "mock grep crew".to_string(),
            mode: "local-only".to_string(),
            backend: "herdr".to_string(),
            verify_cmd: None,
            n: Some(3),
        };
        let res = handler.dispatch(&args).await.unwrap();
        let parsed: Vec<CrewDispatchResult> = serde_json::from_str(&res).unwrap();
        assert_eq!(parsed.len(), 3);
        assert!(parsed[0].stats.visits >= 1);
    }
}
