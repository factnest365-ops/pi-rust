use anyhow::{anyhow, Result};
use chrono::Utc;
use pi_tools::git::{git_worktree_create, git_worktree_merge, git_worktree_remove};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CrewTaskShape {
    Ship,
    Scout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CrewMergeMode {
    LocalOnly,
    DirectPr,
    NoMistakes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CrewBackend {
    Herdr,
    Tmux,
    Worktree,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CrewTaskStatus {
    Working,
    Blocked(String),
    Done(String),
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrewTask {
    pub id: String,
    pub shape: CrewTaskShape,
    pub task: String,
    pub repo_path: PathBuf,
    pub worktree_path: Option<PathBuf>,
    pub branch_name: Option<String>,
    pub mode: CrewMergeMode,
    pub backend: CrewBackend,
    pub verify_cmd: Option<String>,
    pub status: CrewTaskStatus,
    pub outcome: Option<String>,
    pub created_at: String,
    pub finished_at: Option<String>,
}

#[derive(Clone)]
pub struct FirstMateDistro {
    pub repo_root: PathBuf,
    tasks: Arc<RwLock<HashMap<String, CrewTask>>>,
}

impl FirstMateDistro {
    pub fn new<P: AsRef<Path>>(repo_root: P) -> Self {
        Self {
            repo_root: repo_root.as_ref().to_path_buf(),
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Dispatches a new task to the fleet (either a Scout investigation or a Ship worktree task)
    pub async fn dispatch(
        &self,
        shape: CrewTaskShape,
        task: String,
        mode: CrewMergeMode,
        backend: CrewBackend,
        verify_cmd: Option<String>,
    ) -> Result<CrewTask> {
        let task_id = Uuid::new_v4().to_string()[..8].to_string();
        let created_at = Utc::now().to_rfc3339();

        let (worktree_path, branch_name) = match shape {
            CrewTaskShape::Ship => {
                let wt_path = git_worktree_create("HEAD", &task_id)?;
                let branch = format!("pi-task-{}", task_id);
                (Some(wt_path), Some(branch))
            }
            CrewTaskShape::Scout => (None, None),
        };

        let crew_task = CrewTask {
            id: task_id.clone(),
            shape,
            task,
            repo_path: self.repo_root.clone(),
            worktree_path,
            branch_name,
            mode,
            backend,
            verify_cmd,
            status: CrewTaskStatus::Working,
            outcome: None,
            created_at,
            finished_at: None,
        };

        let mut lock = self.tasks.write().await;
        lock.insert(task_id, crew_task.clone());
        Ok(crew_task)
    }

    /// Marks a crew task as completed with an outcome or findings report
    pub async fn complete_task(&self, task_id: &str, outcome: &str) -> Result<()> {
        let mut lock = self.tasks.write().await;
        let task = lock
            .get_mut(task_id)
            .ok_or_else(|| anyhow!("Crew task not found: {}", task_id))?;

        task.status = CrewTaskStatus::Done(outcome.to_string());
        task.outcome = Some(outcome.to_string());
        task.finished_at = Some(Utc::now().to_rfc3339());
        Ok(())
    }

    /// Merges a completed Ship task worktree back into the target branch with optional verification
    pub async fn merge_ship_task(
        &self,
        task_id: &str,
        target_branch: &str,
        custom_verify: Option<&str>,
    ) -> Result<String> {
        let (worktree_path, verify_cmd) = {
            let lock = self.tasks.read().await;
            let task = lock
                .get(task_id)
                .ok_or_else(|| anyhow!("Crew task not found: {}", task_id))?;

            if task.shape != CrewTaskShape::Ship {
                return Err(anyhow!("Cannot merge Scout task (no worktree code changes)"));
            }
            (task.worktree_path.clone(), task.verify_cmd.clone())
        };

        // 1. Run validation pipeline if configured
        let test_cmd = custom_verify.or(verify_cmd.as_deref());
        if let Some(cmd_str) = test_cmd
            && let Some(ref wt) = worktree_path
        {
            let status = Command::new("sh")
                .arg("-c")
                .arg(cmd_str)
                .current_dir(wt)
                .status()
                .map_err(|e| anyhow!("Failed to run verification command '{}': {}", cmd_str, e))?;

            if !status.success() {
                return Err(anyhow!(
                    "Verification command '{}' failed in worktree '{}'. Aborting merge.",
                    cmd_str,
                    wt.display()
                ));
            }
        }

        // 2. Perform git worktree merge
        let merge_output = git_worktree_merge(task_id, target_branch)?;

        // 3. Clean up the disposable worktree
        let _ = git_worktree_remove(task_id, false);

        // 4. Update task state
        {
            let mut lock = self.tasks.write().await;
            if let Some(task) = lock.get_mut(task_id) {
                task.status = CrewTaskStatus::Done(format!("Merged into {}", target_branch));
                task.finished_at = Some(Utc::now().to_rfc3339());
            }
        }

        Ok(merge_output)
    }

    /// Reconciles the status of all crew tasks on disk and in memory
    pub async fn reconcile_fleet(&self) -> Vec<CrewTask> {
        let lock = self.tasks.read().await;
        lock.values().cloned().collect()
    }

    /// Turn-end guard: reports if any background crew work is still active
    pub async fn turn_end_guard(&self) -> (bool, Vec<String>) {
        let lock = self.tasks.read().await;
        let mut active_ids = Vec::new();
        for (id, task) in lock.iter() {
            if matches!(task.status, CrewTaskStatus::Working | CrewTaskStatus::Blocked(_)) {
                active_ids.push(format!("{}: [{:?}] {}", id, task.shape, task.task));
            }
        }
        let has_active = !active_ids.is_empty();
        (has_active, active_ids)
    }

    pub fn create_tool_handler(self: &Arc<Self>) -> Arc<dyn pi_tools::CrewToolHandler> {
        Arc::new(FirstMateToolHandlerBridge {
            distro: self.clone(),
        })
    }

    pub fn init_global_handler(self: &Arc<Self>) {
        let handler = self.create_tool_handler();
        pi_tools::register_crew_handler(handler);
    }
}

pub struct FirstMateToolHandlerBridge {
    pub distro: Arc<FirstMateDistro>,
}

impl pi_tools::CrewToolHandler for FirstMateToolHandlerBridge {
    fn dispatch<'a>(
        &'a self,
        args: &'a pi_tools::CrewDispatchArgs,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            let shape = match args.shape.to_lowercase().as_str() {
                "scout" => CrewTaskShape::Scout,
                _ => CrewTaskShape::Ship,
            };
            let mode = match args.mode.to_lowercase().as_str() {
                "direct-pr" | "pr" => CrewMergeMode::DirectPr,
                "no-mistakes" => CrewMergeMode::NoMistakes,
                _ => CrewMergeMode::LocalOnly,
            };
            let backend = match args.backend.to_lowercase().as_str() {
                "tmux" => CrewBackend::Tmux,
                "worktree" => CrewBackend::Worktree,
                _ => CrewBackend::Herdr,
            };

            let task = self
                .distro
                .dispatch(
                    shape,
                    args.task.clone(),
                    mode,
                    backend,
                    args.verify_cmd.clone(),
                )
                .await?;
            serde_json::to_string_pretty(&task)
                .map_err(|e| anyhow!("Failed to serialize crew task: {}", e))
        })
    }

    fn status<'a>(
        &'a self,
        args: &'a pi_tools::CrewStatusArgs,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            let tasks = self.distro.reconcile_fleet().await;
            if let Some(ref tid) = args.task_id {
                if let Some(task) = tasks.into_iter().find(|t| t.id == *tid) {
                    return serde_json::to_string_pretty(&task)
                        .map_err(|e| anyhow!("Failed to serialize crew task: {}", e));
                } else {
                    return Err(anyhow!("Crew task not found: {}", tid));
                }
            }
            serde_json::to_string_pretty(&tasks)
                .map_err(|e| anyhow!("Failed to serialize fleet status: {}", e))
        })
    }

    fn merge<'a>(
        &'a self,
        args: &'a pi_tools::CrewMergeArgs,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            let res = self
                .distro
                .merge_ship_task(
                    &args.task_id,
                    &args.target_branch,
                    args.verify_cmd.as_deref(),
                )
                .await?;
            Ok(res)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn setup_git_repo(dir: &Path) {
        Command::new("git").args(["init"]).current_dir(dir).output().unwrap();
        Command::new("git").args(["config", "user.name", "Pi FirstMate"]).current_dir(dir).output().unwrap();
        Command::new("git").args(["config", "user.email", "firstmate@pi.dev"]).current_dir(dir).output().unwrap();
        std::fs::write(dir.join("README.md"), "# Test Repo\n").unwrap();
        Command::new("git").args(["add", "README.md"]).current_dir(dir).output().unwrap();
        Command::new("git").args(["commit", "-m", "Initial commit"]).current_dir(dir).output().unwrap();
    }

    #[tokio::test]
    async fn test_firstmate_scout_task_lifecycle() {
        let tmp = tempdir().unwrap();
        let distro = FirstMateDistro::new(tmp.path());

        let task = distro
            .dispatch(
                CrewTaskShape::Scout,
                "Investigate token compaction performance".to_string(),
                CrewMergeMode::LocalOnly,
                CrewBackend::Herdr,
                None,
            )
            .await
            .unwrap();

        assert_eq!(task.shape, CrewTaskShape::Scout);
        assert!(task.worktree_path.is_none());

        let (has_active, active) = distro.turn_end_guard().await;
        assert!(has_active);
        assert_eq!(active.len(), 1);

        distro
            .complete_task(&task.id, "BPE tokenizer is 4x faster with caching")
            .await
            .unwrap();

        let (has_active_after, _) = distro.turn_end_guard().await;
        assert!(!has_active_after);
    }

    #[tokio::test]
    async fn test_firstmate_ship_task_worktree_lifecycle() {
        let tmp = tempdir().unwrap();
        setup_git_repo(tmp.path());

        // Change current directory to temp repo for git worktree operations
        let _guard = std::env::set_current_dir(tmp.path());

        let distro = FirstMateDistro::new(tmp.path());
        let task = distro
            .dispatch(
                CrewTaskShape::Ship,
                "Implement new feature".to_string(),
                CrewMergeMode::LocalOnly,
                CrewBackend::Worktree,
                Some("true".to_string()),
            )
            .await
            .unwrap();

        assert_eq!(task.shape, CrewTaskShape::Ship);
        assert!(task.worktree_path.is_some());

        // Simulate worker making changes in the worktree
        if let Some(ref wt) = task.worktree_path {
            std::fs::write(wt.join("FEATURE.md"), "# Feature Done\n").unwrap();
            Command::new("git").args(["add", "FEATURE.md"]).current_dir(wt).output().unwrap();
            Command::new("git").args(["commit", "-m", "Add feature"]).current_dir(wt).output().unwrap();
        }

        // Merge worktree change back to current branch
        let merge_res = distro.merge_ship_task(&task.id, "HEAD", None).await;
        assert!(merge_res.is_ok(), "Merge failed: {:?}", merge_res);
    }
}
