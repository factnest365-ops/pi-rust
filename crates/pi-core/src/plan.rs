use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{Duration, timeout};
use uuid::Uuid;

pub const VERIFY_TIMEOUT_SECS: u64 = 120;

/// Execution status of an individual task in a plan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Running { progress_pct: u8, started_at: u64 },
    Completed { duration_ms: u64, summary: String },
    Failed { error: String, retry_count: u8 },
}

impl TaskStatus {
    pub fn is_pending(&self) -> bool {
        matches!(self, TaskStatus::Pending)
    }

    pub fn is_running(&self) -> bool {
        matches!(self, TaskStatus::Running { .. })
    }

    pub fn is_completed(&self) -> bool {
        matches!(self, TaskStatus::Completed { .. })
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, TaskStatus::Failed { .. })
    }

    pub fn status_icon(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "[ ]",
            TaskStatus::Running { .. } => "[◐]",
            TaskStatus::Completed { .. } => "[✔]",
            TaskStatus::Failed { .. } => "[✖]",
        }
    }
}

/// An individual unit of work within an ExecutionPlan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanTask {
    pub id: String,
    pub title: String,
    pub description: String,
    pub dependencies: Vec<String>,
    pub status: TaskStatus,
    pub verification_command: Option<String>,
}

impl PlanTask {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: description.into(),
            dependencies: Vec::new(),
            status: TaskStatus::Pending,
            verification_command: None,
        }
    }

    pub fn with_dependencies(mut self, dependencies: Vec<String>) -> Self {
        self.dependencies = dependencies;
        self
    }

    pub fn with_verification(mut self, command: impl Into<String>) -> Self {
        self.verification_command = Some(command.into());
        self
    }

    pub fn is_ready(&self, completed_task_ids: &[String]) -> bool {
        self.status.is_pending()
            && self
                .dependencies
                .iter()
                .all(|dep| completed_task_ids.contains(dep))
    }
}

/// A structured graph of tasks to achieve a high-level goal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionPlan {
    pub id: String,
    pub goal: String,
    pub tasks: Vec<PlanTask>,
    pub active_task_idx: Option<usize>,
}

impl ExecutionPlan {
    pub fn new(id: impl Into<String>, goal: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            goal: goal.into(),
            tasks: Vec::new(),
            active_task_idx: None,
        }
    }

    pub fn add_task(&mut self, task: PlanTask) {
        self.tasks.push(task);
        if self.active_task_idx.is_none() && !self.tasks.is_empty() {
            self.active_task_idx = self.next_ready_task_idx();
        }
    }

    pub fn get_task(&self, id: &str) -> Option<&PlanTask> {
        self.tasks.iter().find(|t| t.id == id)
    }

    pub fn get_task_mut(&mut self, id: &str) -> Option<&mut PlanTask> {
        self.tasks.iter_mut().find(|t| t.id == id)
    }

    pub fn completed_task_ids(&self) -> Vec<String> {
        self.tasks
            .iter()
            .filter(|t| t.status.is_completed())
            .map(|t| t.id.clone())
            .collect()
    }

    pub fn next_ready_task_idx(&self) -> Option<usize> {
        let completed = self.completed_task_ids();
        self.tasks.iter().position(|t| t.is_ready(&completed))
    }

    pub fn is_complete(&self) -> bool {
        !self.tasks.is_empty() && self.tasks.iter().all(|t| t.status.is_completed())
    }

    pub fn is_failed(&self) -> bool {
        self.tasks.iter().any(|t| t.status.is_failed())
    }

    pub fn completion_stats(&self) -> (usize, usize) {
        let completed = self
            .tasks
            .iter()
            .filter(|t| t.status.is_completed())
            .count();
        (completed, self.tasks.len())
    }

    pub fn reset(&mut self) {
        for task in &mut self.tasks {
            task.status = TaskStatus::Pending;
        }
        self.active_task_idx = self.next_ready_task_idx();
    }

    /// Renders an interactive markdown checklist with icons: `[✔]`, `[◐]`, `[ ]`, `[✖]`.
    pub fn to_markdown_checklist(&self) -> String {
        let mut out = format!("### Plan: {}\n\n", self.goal);

        for (idx, task) in self.tasks.iter().enumerate() {
            let icon = task.status.status_icon();
            let num = idx + 1;

            match &task.status {
                TaskStatus::Completed { duration_ms, .. } => {
                    out.push_str(&format!(
                        "- {} {}. **{}** — {} *(Completed in {}ms)*\n",
                        icon, num, task.title, task.description, duration_ms
                    ));
                }
                TaskStatus::Running { progress_pct, .. } => {
                    out.push_str(&format!(
                        "- {} {}. **{}** — {} *(Running: {}%)*\n",
                        icon, num, task.title, task.description, progress_pct
                    ));
                }
                TaskStatus::Failed { error, retry_count } => {
                    let err_snippet = if error.len() > 60 {
                        let boundary = error.floor_char_boundary(60);
                        format!("{}...", &error[..boundary])
                    } else {
                        error.clone()
                    };
                    out.push_str(&format!(
                        "- {} {}. **{}** — {} *(Failed: {}, retries: {})*\n",
                        icon, num, task.title, task.description, err_snippet, retry_count
                    ));
                }
                TaskStatus::Pending => {
                    out.push_str(&format!(
                        "- {} {}. **{}** — {}\n",
                        icon, num, task.title, task.description
                    ));
                }
            }
        }

        let (completed, total) = self.completion_stats();
        let pct = (completed * 100).checked_div(total).unwrap_or(0);

        out.push_str(&format!(
            "\n**Progress:** {}/{} tasks completed ({}%)\n",
            completed, total, pct
        ));
        out
    }
}

/// Orchestrator and state machine for plan execution, automated verification, and self-repair.
#[derive(Debug, Clone)]
pub struct PlanExecutor {
    pub plan: ExecutionPlan,
    pub max_retries: u8,
    pub workspace_path: Option<PathBuf>,
}

impl PlanExecutor {
    pub fn new(plan: ExecutionPlan) -> Self {
        let mut executor = Self {
            plan,
            max_retries: 3,
            workspace_path: None,
        };
        if executor.plan.active_task_idx.is_none() && !executor.plan.tasks.is_empty() {
            executor.plan.active_task_idx = executor.plan.next_ready_task_idx();
        }
        executor
    }

    pub fn with_max_retries(mut self, max_retries: u8) -> Self {
        self.max_retries = max_retries;
        self
    }

    pub fn with_workspace_path(mut self, path: PathBuf) -> Self {
        self.workspace_path = Some(path);
        self
    }

    /// Decomposes a high-level goal into structured tasks.
    /// If the goal text contains list/step structures, they are parsed directly into sequential tasks.
    /// Otherwise, it generates a standard 4-phase architectural task breakdown.
    pub fn decompose_goal(goal: &str, _model: Option<&str>) -> ExecutionPlan {
        let plan_id = Uuid::new_v4().to_string();
        let mut plan = ExecutionPlan::new(plan_id, goal.trim());

        let lines: Vec<&str> = goal
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect();

        // Check if goal has numbered list or checklist items
        let mut parsed_items = Vec::new();
        for line in lines {
            let item_text = if let Some(stripped) = line.strip_prefix("- [ ] ") {
                Some(stripped.trim())
            } else if let Some(stripped) = line.strip_prefix("- ") {
                Some(stripped.trim())
            } else if let Some(stripped) = line.strip_prefix("* ") {
                Some(stripped.trim())
            } else if let Some(idx) = line.find('.')
                && idx > 0
                && idx <= 3
                && line[..idx].chars().all(|c| c.is_ascii_digit())
            {
                Some(line[idx + 1..].trim())
            } else if let Some(stripped) = line.strip_prefix("Step ")
                && let Some(idx) = stripped.find(':')
            {
                Some(stripped[idx + 1..].trim())
            } else {
                None
            };

            if let Some(text) = item_text
                && !text.is_empty()
            {
                parsed_items.push(text.to_string());
            }
        }

        if parsed_items.len() >= 2 {
            let mut prev_id: Option<String> = None;
            for (idx, item) in parsed_items.into_iter().enumerate() {
                let task_id = format!("task-{}", idx + 1);

                // Extract verification command if present (e.g. `[verify: cargo test]`)
                let (cleaned_title, verification_cmd) = Self::extract_verification_marker(&item);

                let mut task = PlanTask::new(
                    &task_id,
                    cleaned_title.clone(),
                    format!("Execute step: {}", cleaned_title),
                );

                if let Some(ref prev) = prev_id {
                    task.dependencies.push(prev.clone());
                }

                if let Some(cmd) = verification_cmd {
                    task.verification_command = Some(cmd);
                }

                prev_id = Some(task_id);
                plan.add_task(task);
            }
        } else {
            // Default 4-phase structured engineering breakdown
            let t1 = PlanTask::new(
                "task-1",
                "Specification & Architecture",
                format!(
                    "Define interface boundaries and plan changes for: {}",
                    goal.trim()
                ),
            );

            let t2 = PlanTask::new(
                "task-2",
                "Implementation & Code Changes",
                format!("Implement the core changes to satisfy: {}", goal.trim()),
            )
            .with_dependencies(vec!["task-1".to_string()]);

            let t3 = PlanTask::new(
                "task-3",
                "Unit & Integration Verification",
                "Execute automated test suite to ensure no regressions",
            )
            .with_dependencies(vec!["task-2".to_string()])
            .with_verification("cargo test");

            let t4 = PlanTask::new(
                "task-4",
                "Quality & Compilation Gate",
                "Verify workspace type checking and compiler warnings",
            )
            .with_dependencies(vec!["task-3".to_string()])
            .with_verification("cargo check");

            plan.add_task(t1);
            plan.add_task(t2);
            plan.add_task(t3);
            plan.add_task(t4);
        }

        plan.active_task_idx = plan.next_ready_task_idx();
        plan
    }

    fn extract_verification_marker(input: &str) -> (String, Option<String>) {
        // Match markers like `[verify: cargo test]` or `(cmd: npm test)`
        let lower = input.to_lowercase();
        if let Some(v_idx) = lower.find("[verify:")
            && let Some(end_idx) = input[v_idx..].find(']')
        {
            let cmd = input[v_idx + 8..v_idx + end_idx].trim().to_string();
            let mut title = input[..v_idx].trim().to_string();
            let rest = input[v_idx + end_idx + 1..].trim();
            if !rest.is_empty() {
                title.push(' ');
                title.push_str(rest);
            }
            return (title, if cmd.is_empty() { None } else { Some(cmd) });
        }

        if let Some(v_idx) = lower.find("(cmd:")
            && let Some(end_idx) = input[v_idx..].find(')')
        {
            let cmd = input[v_idx + 5..v_idx + end_idx].trim().to_string();
            let mut title = input[..v_idx].trim().to_string();
            let rest = input[v_idx + end_idx + 1..].trim();
            if !rest.is_empty() {
                title.push(' ');
                title.push_str(rest);
            }
            return (title, if cmd.is_empty() { None } else { Some(cmd) });
        }

        (input.to_string(), None)
    }

    /// Executes the active task in the plan.
    /// Transitions task to Running, runs `verification_command` via `tokio::process::Command` if present,
    /// and marks task Completed on success or Failed on verification failure.
    pub async fn execute_next_task(&mut self) -> Result<Option<PlanTask>> {
        let task_idx = match self.plan.active_task_idx {
            Some(idx)
                if idx < self.plan.tasks.len() && self.plan.tasks[idx].status.is_pending() =>
            {
                idx
            }
            _ => match self.plan.next_ready_task_idx() {
                Some(ready_idx) => {
                    self.plan.active_task_idx = Some(ready_idx);
                    ready_idx
                }
                None => {
                    self.plan.active_task_idx = None;
                    return Ok(None);
                }
            },
        };

        let started_at = current_epoch_ms();
        self.plan.tasks[task_idx].status = TaskStatus::Running {
            progress_pct: 0,
            started_at,
        };

        let verification_cmd = self.plan.tasks[task_idx].verification_command.clone();
        let task_title = self.plan.tasks[task_idx].title.clone();

        if let Some(cmd_str) = verification_cmd {
            let start_instant = std::time::Instant::now();
            let working_dir = self
                .workspace_path
                .clone()
                .or_else(|| std::env::current_dir().ok());

            let exec_future = async {
                let mut cmd = if cfg!(target_os = "windows") {
                    let mut c = tokio::process::Command::new("cmd");
                    c.arg("/C").arg(&cmd_str);
                    c
                } else {
                    let mut c = tokio::process::Command::new("sh");
                    c.arg("-c").arg(&cmd_str);
                    c
                };

                if let Some(ref dir) = working_dir {
                    cmd.current_dir(dir);
                }

                cmd.stdout(Stdio::piped());
                cmd.stderr(Stdio::piped());
                cmd.kill_on_drop(true);

                let child = cmd.spawn().with_context(|| {
                    format!("Failed to spawn verification command: {}", cmd_str)
                })?;

                // Invariant 5: Subprocess Safety & Timeout Guarantees (`VERIFY_TIMEOUT_SECS` timeout)
                let output = match timeout(
                    Duration::from_secs(VERIFY_TIMEOUT_SECS),
                    child.wait_with_output(),
                )
                .await
                {
                    Ok(Ok(output)) => output,
                    Ok(Err(e)) => {
                        return Err(anyhow::anyhow!("Verification execution error: {}", e));
                    }
                    Err(_) => {
                        return Err(anyhow::anyhow!(format!(
                            "Verification command timed out after {}s",
                            VERIFY_TIMEOUT_SECS
                        )));
                    }
                };
                Ok::<std::process::Output, anyhow::Error>(output)
            };

            let result = exec_future.await;
            let duration_ms = start_instant.elapsed().as_millis() as u64;

            match result {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let combined = format!("{}{}", stdout, stderr);

                    // UTF-8 character boundary safe truncation (Invariant 9)
                    let snippet_len = combined.floor_char_boundary(combined.len().min(1000));
                    let snippet = combined[..snippet_len].trim();

                    if output.status.success() {
                        let summary = if snippet.is_empty() {
                            format!("Task '{}' verified successfully", task_title)
                        } else {
                            format!("Verification passed: {}", snippet)
                        };

                        self.plan.tasks[task_idx].status = TaskStatus::Completed {
                            duration_ms,
                            summary,
                        };
                        self.plan.active_task_idx = self.plan.next_ready_task_idx();
                    } else {
                        let code = output.status.code().unwrap_or(-1);
                        let error = format!(
                            "Verification failed with exit code {}: {}",
                            code,
                            if snippet.is_empty() {
                                "Command returned non-zero status"
                            } else {
                                snippet
                            }
                        );

                        self.plan.tasks[task_idx].status = TaskStatus::Failed {
                            error,
                            retry_count: 0,
                        };
                    }
                }
                Err(e) => {
                    self.plan.tasks[task_idx].status = TaskStatus::Failed {
                        error: format!("Verification execution error: {}", e),
                        retry_count: 0,
                    };
                }
            }
        } else {
            // Task has no verification command; mark completed immediately
            let duration_ms = 0;
            self.plan.tasks[task_idx].status = TaskStatus::Completed {
                duration_ms,
                summary: format!("Task '{}' completed", task_title),
            };
            self.plan.active_task_idx = self.plan.next_ready_task_idx();
        }

        Ok(Some(self.plan.tasks[task_idx].clone()))
    }

    /// Handles failure recovery for a task.
    /// Resets status to Pending if retry_count < max_retries and sets active_task_idx.
    /// Returns Ok(true) if retry was scheduled, Ok(false) if max retries exceeded.
    pub fn retry_or_repair(&mut self, task_id: &str) -> Result<bool> {
        let task_idx = self
            .plan
            .tasks
            .iter()
            .position(|t| t.id == task_id)
            .ok_or_else(|| anyhow!("Task with id '{}' not found in plan", task_id))?;

        let task = &mut self.plan.tasks[task_idx];
        match task.status {
            TaskStatus::Failed {
                ref error,
                retry_count,
            } => {
                if retry_count < self.max_retries {
                    let _next_retry = retry_count + 1;
                    task.status = TaskStatus::Pending;
                    // If we retry, point active task to this task
                    self.plan.active_task_idx = Some(task_idx);
                    Ok(true)
                } else {
                    let _ = error;
                    Ok(false)
                }
            }
            _ => Ok(true),
        }
    }

    /// Generates diagnostic self-healing prompt if a task failed verification.
    pub fn generate_repair_prompt(&self, task_id: &str) -> Option<String> {
        let task = self.plan.get_task(task_id)?;
        if let TaskStatus::Failed {
            ref error,
            retry_count,
        } = task.status
        {
            Some(format!(
                "[Task Verification Failure Detected]\n\
                 Task ID: {}\n\
                 Title: {}\n\
                 Description: {}\n\
                 Verification Command: {}\n\
                 Retry Count: {} / {}\n\
                 Failure Output:\n\
                 {}\n\n\
                 Please inspect the diagnostics above, repair the root cause, and re-verify.",
                task.id,
                task.title,
                task.description,
                task.verification_command.as_deref().unwrap_or("N/A"),
                retry_count,
                self.max_retries,
                error
            ))
        } else {
            None
        }
    }

    /// Renders an interactive markdown checklist with icons: `[✔]`, `[◐]`, `[ ]`, `[✖]`.
    pub fn to_markdown_checklist(&self) -> String {
        let mut out = format!("### Plan: {}\n\n", self.plan.goal);

        for (idx, task) in self.plan.tasks.iter().enumerate() {
            let icon = task.status.status_icon();
            let num = idx + 1;

            match &task.status {
                TaskStatus::Completed { duration_ms, .. } => {
                    out.push_str(&format!(
                        "- {} {}. **{}** — {} *(Completed in {}ms)*\n",
                        icon, num, task.title, task.description, duration_ms
                    ));
                }
                TaskStatus::Running { progress_pct, .. } => {
                    out.push_str(&format!(
                        "- {} {}. **{}** — {} *(Running: {}%)*\n",
                        icon, num, task.title, task.description, progress_pct
                    ));
                }
                TaskStatus::Failed { error, retry_count } => {
                    let err_snippet = if error.len() > 60 {
                        let boundary = error.floor_char_boundary(60);
                        format!("{}...", &error[..boundary])
                    } else {
                        error.clone()
                    };
                    out.push_str(&format!(
                        "- {} {}. **{}** — {} *(Failed: {}, retries: {})*\n",
                        icon, num, task.title, task.description, err_snippet, retry_count
                    ));
                }
                TaskStatus::Pending => {
                    out.push_str(&format!(
                        "- {} {}. **{}** — {}\n",
                        icon, num, task.title, task.description
                    ));
                }
            }
        }

        let (completed, total) = self.plan.completion_stats();
        let pct = (completed * 100).checked_div(total).unwrap_or(0);

        out.push_str(&format!(
            "\n**Progress:** {}/{} tasks completed ({}%)\n",
            completed, total, pct
        ));
        out
    }
}

fn current_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_status_predicates_and_icons() {
        let pending = TaskStatus::Pending;
        assert!(pending.is_pending());
        assert_eq!(pending.status_icon(), "[ ]");

        let running = TaskStatus::Running {
            progress_pct: 45,
            started_at: 1000,
        };
        assert!(running.is_running());
        assert_eq!(running.status_icon(), "[◐]");

        let completed = TaskStatus::Completed {
            duration_ms: 120,
            summary: "Done".to_string(),
        };
        assert!(completed.is_completed());
        assert_eq!(completed.status_icon(), "[✔]");

        let failed = TaskStatus::Failed {
            error: "Cargo error".to_string(),
            retry_count: 1,
        };
        assert!(failed.is_failed());
        assert_eq!(failed.status_icon(), "[✖]");
    }

    #[test]
    fn test_plan_task_dependencies_and_readiness() {
        let t1 = PlanTask::new("t1", "Task 1", "First step");
        let t2 =
            PlanTask::new("t2", "Task 2", "Second step").with_dependencies(vec!["t1".to_string()]);

        let completed = vec![];
        assert!(t1.is_ready(&completed));
        assert!(!t2.is_ready(&completed));

        let completed = vec!["t1".to_string()];
        assert!(t2.is_ready(&completed));
    }

    #[test]
    fn test_execution_plan_stats_and_completion() {
        let mut plan = ExecutionPlan::new("p1", "Build web crawler");
        assert_eq!(plan.completion_stats(), (0, 0));
        assert!(!plan.is_complete());

        let t1 = PlanTask::new("t1", "Design", "Spec interfaces");
        let t2 = PlanTask::new("t2", "Code", "Write logic");
        plan.add_task(t1);
        plan.add_task(t2);

        assert_eq!(plan.completion_stats(), (0, 2));
        assert_eq!(plan.active_task_idx, Some(0));

        plan.tasks[0].status = TaskStatus::Completed {
            duration_ms: 50,
            summary: "Done".to_string(),
        };
        assert_eq!(plan.completion_stats(), (1, 2));
        assert!(!plan.is_complete());

        plan.tasks[1].status = TaskStatus::Completed {
            duration_ms: 70,
            summary: "Done".to_string(),
        };
        assert_eq!(plan.completion_stats(), (2, 2));
        assert!(plan.is_complete());
    }

    #[test]
    fn test_decompose_goal_structured_lines() {
        let goal = r#"
1. Design parser [verify: cargo check]
2. Implement AST [verify: cargo test]
3. Add benchmark suite
"#;
        let plan = PlanExecutor::decompose_goal(goal, None);
        assert_eq!(plan.tasks.len(), 3);
        assert_eq!(plan.tasks[0].id, "task-1");
        assert_eq!(plan.tasks[0].title, "Design parser");
        assert_eq!(
            plan.tasks[0].verification_command,
            Some("cargo check".to_string())
        );

        assert_eq!(plan.tasks[1].id, "task-2");
        assert_eq!(plan.tasks[1].title, "Implement AST");
        assert_eq!(plan.tasks[1].dependencies, vec!["task-1".to_string()]);
        assert_eq!(
            plan.tasks[1].verification_command,
            Some("cargo test".to_string())
        );

        assert_eq!(plan.tasks[2].id, "task-3");
        assert_eq!(plan.tasks[2].title, "Add benchmark suite");
        assert_eq!(plan.tasks[2].dependencies, vec!["task-2".to_string()]);
        assert_eq!(plan.tasks[2].verification_command, None);
    }

    #[test]
    fn test_decompose_goal_checklist_markers() {
        let goal = r#"
- [ ] Initialize repository (cmd: git status)
- [ ] Setup CI pipeline [verify: cargo test]
"#;
        let plan = PlanExecutor::decompose_goal(goal, None);
        assert_eq!(plan.tasks.len(), 2);
        assert_eq!(plan.tasks[0].title, "Initialize repository");
        assert_eq!(
            plan.tasks[0].verification_command,
            Some("git status".to_string())
        );
        assert_eq!(plan.tasks[1].title, "Setup CI pipeline");
        assert_eq!(
            plan.tasks[1].verification_command,
            Some("cargo test".to_string())
        );
    }

    #[test]
    fn test_decompose_goal_fallback_engineering_phases() {
        let goal = "Refactor database pool connection handling";
        let plan = PlanExecutor::decompose_goal(goal, None);
        assert_eq!(plan.tasks.len(), 4);
        assert_eq!(plan.tasks[0].id, "task-1");
        assert_eq!(plan.tasks[0].title, "Specification & Architecture");
        assert_eq!(plan.tasks[1].id, "task-2");
        assert_eq!(plan.tasks[2].id, "task-3");
        assert_eq!(
            plan.tasks[2].verification_command,
            Some("cargo test".to_string())
        );
        assert_eq!(plan.tasks[3].id, "task-4");
        assert_eq!(
            plan.tasks[3].verification_command,
            Some("cargo check".to_string())
        );
    }

    #[test]
    fn test_to_markdown_checklist() {
        let mut plan = ExecutionPlan::new("p1", "Test Markdown Checklist");
        let mut t1 = PlanTask::new("t1", "Init Repo", "Create folders");
        t1.status = TaskStatus::Completed {
            duration_ms: 125,
            summary: "Done".to_string(),
        };
        let mut t2 = PlanTask::new("t2", "Write Code", "Implement logic");
        t2.status = TaskStatus::Running {
            progress_pct: 50,
            started_at: 1000,
        };
        let t3 = PlanTask::new("t3", "Verification", "Run tests");
        let mut t4 = PlanTask::new("t4", "Deploy", "Release version");
        t4.status = TaskStatus::Failed {
            error: "Connection refused".to_string(),
            retry_count: 1,
        };

        plan.add_task(t1);
        plan.add_task(t2);
        plan.add_task(t3);
        plan.add_task(t4);

        let executor = PlanExecutor::new(plan);
        let md = executor.to_markdown_checklist();

        assert!(md.contains("### Plan: Test Markdown Checklist"));
        assert!(md.contains("[✔] 1. **Init Repo** — Create folders *(Completed in 125ms)*"));
        assert!(md.contains("[◐] 2. **Write Code** — Implement logic *(Running: 50%)*"));
        assert!(md.contains("[ ] 3. **Verification** — Run tests"));
        assert!(md.contains(
            "[✖] 4. **Deploy** — Release version *(Failed: Connection refused, retries: 1)*"
        ));
        assert!(md.contains("**Progress:** 1/4 tasks completed (25%)"));
    }

    #[tokio::test]
    async fn test_execute_next_task_with_success_and_failure() {
        let mut plan = ExecutionPlan::new("p_test", "Async verification test");
        let t1 = PlanTask::new("t1", "Echo Test", "Run simple echo")
            .with_verification("echo 'tau test pass'");
        let t2 = PlanTask::new("t2", "Failing Command", "Run false")
            .with_dependencies(vec!["t1".to_string()])
            .with_verification("false");
        plan.add_task(t1);
        plan.add_task(t2);

        let mut executor = PlanExecutor::new(plan);

        // Execute task 1 (echo 'tau test pass') -> should succeed
        let res1 = executor.execute_next_task().await.unwrap();
        assert!(res1.is_some());
        let executed_t1 = res1.unwrap();
        assert_eq!(executed_t1.id, "t1");
        assert!(executed_t1.status.is_completed());

        // Execute task 2 (false) -> should fail
        let res2 = executor.execute_next_task().await.unwrap();
        assert!(res2.is_some());
        let executed_t2 = res2.unwrap();
        assert_eq!(executed_t2.id, "t2");
        assert!(executed_t2.status.is_failed());

        // Test repair prompt generation
        let repair_prompt = executor.generate_repair_prompt("t2");
        assert!(repair_prompt.is_some());
        assert!(
            repair_prompt
                .unwrap()
                .contains("[Task Verification Failure Detected]")
        );

        // Retry recovery
        let can_retry = executor.retry_or_repair("t2").unwrap();
        assert!(can_retry);
        assert!(executor.plan.get_task("t2").unwrap().status.is_pending());
    }
}
