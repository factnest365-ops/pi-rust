use anyhow::{anyhow, Result};
use chrono::Utc;
use pi_tools::git::{
    git_worktree_create_in_dir, git_worktree_merge_in_dir, git_worktree_remove_in_dir,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::SkillRegistry;
use crate::TauVault;

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

#[derive(Debug, Clone, Default)]
pub struct CrewDispatchMemories {
    pub skill_registry: Option<SkillRegistry>,
    pub vault: Option<TauVault>,
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
        self.dispatch_with_memories(shape, task, mode, backend, verify_cmd, None)
            .await
    }

    pub async fn dispatch_with_memories(
        &self,
        shape: CrewTaskShape,
        task: String,
        mode: CrewMergeMode,
        backend: CrewBackend,
        verify_cmd: Option<String>,
        memories: Option<CrewDispatchMemories>,
    ) -> Result<CrewTask> {
        let task_id = Uuid::new_v4().to_string()[..8].to_string();
        let created_at = Utc::now().to_rfc3339();

        let (worktree_path, branch_name) = match shape {
            CrewTaskShape::Ship => {
                let wt_path = git_worktree_create_in_dir("HEAD", &task_id, Some(&self.repo_root))?;
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
            finished_at: None,
            created_at,
        };

        if let Some(ref memories) = memories
            && let Some(ref registry) = memories.skill_registry {
                let input = crate::crew_memory::CrewPrefetchInput {
                    task_text: crew_task.task.clone(),
                    crew_id: None,
                    task_id: Some(crew_task.id.clone()),
                    skill_limit: Some(crate::crew_memory::DEFAULT_CREW_SKILL_LIMIT),
                    memory_limit: Some(5),
                };
                let _ = crate::crew_memory::prefetch_crew_context(
                    memories
                        .vault
                        .as_ref()
                        .unwrap_or(&crate::vault::TauVault::new()),
                    registry,
                    input,
                );
            }

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
            let mut cmd = tokio::process::Command::new("sh");
            cmd.arg("-c").arg(cmd_str).current_dir(wt);
            let mut child = cmd.spawn().map_err(|e| anyhow!("Failed to spawn verification command '{}': {}", cmd_str, e))?;
            let status = tokio::time::timeout(std::time::Duration::from_secs(120), child.wait())
                .await
                .map_err(|_| {
                    let _ = child.start_kill();
                    anyhow!("Verification command '{}' timed out after 120 seconds", cmd_str)
                })?
                .map_err(|e| anyhow!("Verification command execution failed: {}", e))?;

            if !status.success() {
                return Err(anyhow!(
                    "Verification command '{}' failed in worktree '{}'. Aborting merge.",
                    cmd_str,
                    wt.display()
                ));
            }
        }

        // 2. Perform git worktree merge in repo_root
        let merge_output = git_worktree_merge_in_dir(task_id, target_branch, Some(&self.repo_root))?;

        // 3. Clean up the disposable worktree in repo_root
        let _ = git_worktree_remove_in_dir(task_id, false, Some(&self.repo_root));

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

    /// Cancels a running or pending crew task and cleans up any allocated worktree
    pub async fn cancel_task(&self, task_id: &str) -> Result<()> {
        let (worktree_path, shape) = {
            let mut lock = self.tasks.write().await;
            let task = lock
                .get_mut(task_id)
                .ok_or_else(|| anyhow!("Crew task not found: {}", task_id))?;

            task.status = CrewTaskStatus::Failed("Cancelled by user".to_string());
            task.finished_at = Some(Utc::now().to_rfc3339());
            (task.worktree_path.clone(), task.shape)
        };

        if shape == CrewTaskShape::Ship && worktree_path.is_some() {
            let _ = git_worktree_remove_in_dir(task_id, true, Some(&self.repo_root));
        }

        Ok(())
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
            let action = args.action.to_lowercase();
            if action == "cancel" {
                let tid = args
                    .task_id
                    .as_deref()
                    .ok_or_else(|| anyhow!("Missing 'task_id' for cancel action"))?;
                self.distro.cancel_task(tid).await?;
                return Ok(format!("Crew task {} cancelled", tid));
            }

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
    use std::process::Command;
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

    #[tokio::test]
    async fn test_firstmate_verification_command_failure_aborts_merge() {
        let tmp = tempdir().unwrap();
        setup_git_repo(tmp.path());

        let distro = FirstMateDistro::new(tmp.path());
        let task = distro
            .dispatch(
                CrewTaskShape::Ship,
                "Broken feature".to_string(),
                CrewMergeMode::LocalOnly,
                CrewBackend::Worktree,
                Some("exit 1".to_string()),
            )
            .await
            .unwrap();

        let merge_res = distro.merge_ship_task(&task.id, "HEAD", None).await;
        assert!(merge_res.is_err());
        assert!(merge_res.unwrap_err().to_string().contains("Aborting merge"));
    }

    #[tokio::test]
    async fn test_firstmate_cannot_merge_scout_task() {
        let tmp = tempdir().unwrap();
        let distro = FirstMateDistro::new(tmp.path());

        let task = distro
            .dispatch(
                CrewTaskShape::Scout,
                "Read-only scout".to_string(),
                CrewMergeMode::LocalOnly,
                CrewBackend::Herdr,
                None,
            )
            .await
            .unwrap();

        let merge_res = distro.merge_ship_task(&task.id, "HEAD", None).await;
        assert!(merge_res.is_err());
        assert!(merge_res.unwrap_err().to_string().contains("Cannot merge Scout task"));
    }

    #[tokio::test]
    async fn test_firstmate_cancel_task_and_bridge() {
        let tmp = tempdir().unwrap();
        setup_git_repo(tmp.path());

        let distro = Arc::new(FirstMateDistro::new(tmp.path()));
        let handler = distro.create_tool_handler();

        let dispatch_args = pi_tools::CrewDispatchArgs {
            shape: "ship".to_string(),
            task: "Task to cancel".to_string(),
            mode: "local-only".to_string(),
            backend: "worktree".to_string(),
            verify_cmd: None,
        };

        let dispatch_res = handler.dispatch(&dispatch_args).await.unwrap();
        let created_task: CrewTask = serde_json::from_str(&dispatch_res).unwrap();

        // Check status query
        let status_args = pi_tools::CrewStatusArgs {
            task_id: Some(created_task.id.clone()),
            action: "list".to_string(),
        };
        let status_res = handler.status(&status_args).await.unwrap();
        assert!(status_res.contains(&created_task.id));

        // Cancel task via handler
        let cancel_args = pi_tools::CrewStatusArgs {
            task_id: Some(created_task.id.clone()),
            action: "cancel".to_string(),
        };
        let cancel_res = handler.status(&cancel_args).await.unwrap();
        assert!(cancel_res.contains("cancelled"));

        // Fleet status
        let all_tasks = distro.reconcile_fleet().await;
        assert_eq!(all_tasks.len(), 1);
        assert!(matches!(all_tasks[0].status, CrewTaskStatus::Failed(_)));
    }
}
