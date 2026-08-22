use anyhow::{anyhow, Result};
use pi_providers::ModelConfig;
use pi_session::{Role, SessionTree};
use pi_tools::git::{
    git_merge_branch_in_dir, git_worktree_create_at, git_worktree_remove_path,
};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeculativeStrategy {
    pub id: String,
    pub name: String,
    pub prompt_directive: String,
}

impl SpeculativeStrategy {
    pub fn default_strategies() -> Vec<Self> {
        vec![
            Self {
                id: "spec-a".to_string(),
                name: "Approach A: High-Performance / Zero-Alloc".to_string(),
                prompt_directive: "Focus on high performance, zero-allocation algorithms, minimal dependencies, and direct execution. Optimize for speed and memory efficiency.".to_string(),
            },
            Self {
                id: "spec-b".to_string(),
                name: "Approach B: Modular / Clean Architecture".to_string(),
                prompt_directive: "Focus on clean architecture, strong domain abstractions, clear error handling, modularity, and high testability.".to_string(),
            },
            Self {
                id: "crew-grep-first".to_string(),
                name: "Ghost Crew A: Grep-First".to_string(),
                prompt_directive: "Start by grepping for relevant symbols, tests, and call sites before reading or editing. Minimize context and target exact implementation points.".to_string(),
            },
            Self {
                id: "crew-read-first".to_string(),
                name: "Ghost Crew B: Read-First".to_string(),
                prompt_directive: "Read full module boundaries and related files first, then implement with strong typing and explicit error handling.".to_string(),
            },
            Self {
                id: "crew-plan-first".to_string(),
                name: "Ghost Crew C: Plan-First".to_string(),
                prompt_directive: "Write a short plan or checklist, then implement the smallest change that satisfies it and keeps tests green.".to_string(),
            },
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SpeculativeStatus {
    Passed,
    Failed(String),
    VerificationFailed { error: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeculativeBranchResult {
    pub strategy_id: String,
    pub strategy_name: String,
    pub branch_name: String,
    pub worktree_path: PathBuf,
    pub status: SpeculativeStatus,
    pub output_text: String,
    pub lines_added: usize,
    pub lines_removed: usize,
    pub verification_passed: bool,
    pub execution_duration_ms: u64,
    pub diff: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ArbitrationDecision {
    AutoMerged {
        winner_id: String,
        branch_name: String,
        merge_output: String,
    },
    BothPassedSplitDiff {
        recommended_winner: Option<String>,
        diff_a: String,
        diff_b: String,
    },
    BothFailed {
        error: String,
    },
    NoChanges,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeculativeRaceResult {
    pub goal: String,
    pub results: Vec<SpeculativeBranchResult>,
    pub decision: ArbitrationDecision,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpeculativeCrewResult {
    pub crew_id: String,
    pub strategy_id: String,
    pub strategy_name: String,
    pub branch_name: String,
    pub worktree_path: PathBuf,
    pub passed: bool,
    pub reward: f64,
    pub execution_duration_ms: u64,
    pub verification_output: String,
    pub diff: String,
    pub lines_added: usize,
    pub lines_removed: usize,
    pub session_node_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MctsRewardSummary {
    pub total_simulations: usize,
    pub best_crew_id: String,
    pub best_reward: f64,
    pub rewards: Vec<(String, f64)>,
}

#[derive(Clone)]
pub struct SpeculativeEngine {
    pub repo_root: PathBuf,
    pub base_branch: String,
    pub target_branch: String,
    pub verification_command: Option<String>,
}

struct VerificationResult {
    passed: bool,
    output: String,
}

impl SpeculativeEngine {
    pub fn new<P: AsRef<Path>>(repo_root: P) -> Self {
        Self {
            repo_root: repo_root.as_ref().to_path_buf(),
            base_branch: "HEAD".to_string(),
            target_branch: "HEAD".to_string(),
            verification_command: None,
        }
    }

    pub fn with_base_branch(mut self, branch: impl Into<String>) -> Self {
        self.base_branch = branch.into();
        self
    }

    pub fn with_target_branch(mut self, branch: impl Into<String>) -> Self {
        self.target_branch = branch.into();
        self
    }

    pub fn with_verification_cmd(mut self, cmd: impl Into<String>) -> Self {
        self.verification_command = Some(cmd.into());
        self
    }

    /// Runs a speculative execution race using the default AgentLoop runner
    pub async fn run_speculative_race(
        &self,
        goal: &str,
        model_cfg: &ModelConfig,
        strategies: Option<Vec<SpeculativeStrategy>>,
    ) -> Result<SpeculativeRaceResult> {
        let cloned_cfg = model_cfg.clone();
        self.run_speculative_race_with_runner(
            goal,
            model_cfg,
            strategies,
            move |_worktree_path, strategy, goal_text, cfg| {
                let m_cfg = if cfg.provider.is_empty() {
                    cloned_cfg.clone()
                } else {
                    cfg
                };
                async move {
                    let mut agent = crate::AgentLoop::new(m_cfg);
                    let prompt = format!(
                        "Speculative Task Goal: {}\n\nStrategy Directive ({}):\n{}\n\n\
                        Please implement the required solution in this workspace following this strategy directive.\n\
                        Verify your implementation before completion.",
                        goal_text, strategy.name, strategy.prompt_directive
                    );

                    let res = agent.run_turn(&prompt, |_| {}).await?;
                    Ok(res)
                }
            },
        )
        .await
    }

    /// Runs a speculative execution race with a customized task runner
    pub async fn run_speculative_race_with_runner<F, Fut>(
        &self,
        goal: &str,
        model_cfg: &ModelConfig,
        strategies: Option<Vec<SpeculativeStrategy>>,
        runner: F,
    ) -> Result<SpeculativeRaceResult>
    where
        F: Fn(PathBuf, SpeculativeStrategy, String, ModelConfig) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<String>> + Send + 'static,
    {
        // 1. Verify Git Repository
        let git_dir = self.repo_root.join(".git");
        if !git_dir.exists() && !self.repo_root.join("../.git").exists() {
            return Err(anyhow!(
                "Repository at '{}' is not a valid Git repository",
                self.repo_root.display()
            ));
        }

        // 2. Prepare Competing Strategies
        let strats = strategies.unwrap_or_else(SpeculativeStrategy::default_strategies);
        if strats.len() < 2 {
            return Err(anyhow!("Speculative race requires at least 2 strategies"));
        }
        let strat_a = strats[0].clone();
        let strat_b = strats[1].clone();

        let tau_dir = self.repo_root.join(".tau");
        let worktrees_parent = tau_dir.join("worktrees");
        std::fs::create_dir_all(&worktrees_parent)?;

        let tau_gitignore = tau_dir.join(".gitignore");
        if !tau_gitignore.exists() {
            let _ = std::fs::write(&tau_gitignore, "*\n!.gitignore\n");
        }

        let worktree_a_path = worktrees_parent.join(&strat_a.id);
        let worktree_b_path = worktrees_parent.join(&strat_b.id);
        let branch_a_name = format!("tau-{}", &strat_a.id);
        let branch_b_name = format!("tau-{}", &strat_b.id);

        // Clean up prior worktrees if any exist
        let _ = git_worktree_remove_path(&worktree_a_path, Some(&branch_a_name), true, Some(&self.repo_root));
        let _ = git_worktree_remove_path(&worktree_b_path, Some(&branch_b_name), true, Some(&self.repo_root));

        // 3. Fork Phase: Create Ephemeral Git Worktrees
        git_worktree_create_at(&self.base_branch, &branch_a_name, &worktree_a_path, Some(&self.repo_root))?;
        git_worktree_create_at(&self.base_branch, &branch_b_name, &worktree_b_path, Some(&self.repo_root))?;

        let runner_arc = Arc::new(runner);

        // 4. Race Phase: Execute Approach A vs Approach B Concurrently
        let runner_a = runner_arc.clone();
        let wt_a = worktree_a_path.clone();
        let s_a = strat_a.clone();
        let g_a = goal.to_string();
        let cfg_a = model_cfg.clone();

        let task_a = async move {
            let start = Instant::now();
            let res = runner_a(wt_a, s_a, g_a, cfg_a).await;
            (res, start.elapsed().as_millis() as u64)
        };

        let runner_b = runner_arc.clone();
        let wt_b = worktree_b_path.clone();
        let s_b = strat_b.clone();
        let g_b = goal.to_string();
        let cfg_b = model_cfg.clone();

        let task_b = async move {
            let start = Instant::now();
            let res = runner_b(wt_b, s_b, g_b, cfg_b).await;
            (res, start.elapsed().as_millis() as u64)
        };

        let ((res_a, duration_a), (res_b, duration_b)) = tokio::join!(task_a, task_b);

        // 5. Verification Phase: Parallel Automated Verification & Diff Extraction
        let verify_cmd_opt = self.verification_command.clone();
        let v_cmd_a = verify_cmd_opt.clone();
        let v_cmd_b = verify_cmd_opt.clone();

        let wt_a_for_v = worktree_a_path.clone();
        let wt_b_for_v = worktree_b_path.clone();

        let verify_a = Self::verify_workspace(wt_a_for_v, v_cmd_a);
        let verify_b = Self::verify_workspace(wt_b_for_v, v_cmd_b);

        let (ver_a, ver_b) = tokio::join!(verify_a, verify_b);

        let diff_a = Self::get_branch_diff(&worktree_a_path, &self.base_branch);
        let diff_b = Self::get_branch_diff(&worktree_b_path, &self.base_branch);

        let (lines_add_a, lines_del_a) = Self::count_diff_lines(&diff_a);
        let (lines_add_b, lines_del_b) = Self::count_diff_lines(&diff_b);

        let status_a = match &res_a {
            Ok(_) if ver_a.passed => SpeculativeStatus::Passed,
            Ok(_) => SpeculativeStatus::VerificationFailed {
                error: ver_a.output.clone(),
            },
            Err(e) => SpeculativeStatus::Failed(e.to_string()),
        };

        let status_b = match &res_b {
            Ok(_) if ver_b.passed => SpeculativeStatus::Passed,
            Ok(_) => SpeculativeStatus::VerificationFailed {
                error: ver_b.output.clone(),
            },
            Err(e) => SpeculativeStatus::Failed(e.to_string()),
        };

        let passed_a = res_a.is_ok() && ver_a.passed;
        let passed_b = res_b.is_ok() && ver_b.passed;

        let err_a = res_a.as_ref().err().map(|e| e.to_string()).unwrap_or_else(|| ver_a.output.clone());
        let err_b = res_b.as_ref().err().map(|e| e.to_string()).unwrap_or_else(|| ver_b.output.clone());

        let branch_res_a = SpeculativeBranchResult {
            strategy_id: strat_a.id.clone(),
            strategy_name: strat_a.name.clone(),
            branch_name: branch_a_name.clone(),
            worktree_path: worktree_a_path.clone(),
            status: status_a.clone(),
            output_text: res_a.unwrap_or_else(|e| format!("Execution error: {}", e)),
            lines_added: lines_add_a,
            lines_removed: lines_del_a,
            verification_passed: passed_a,
            execution_duration_ms: duration_a,
            diff: diff_a.clone(),
        };

        let branch_res_b = SpeculativeBranchResult {
            strategy_id: strat_b.id.clone(),
            strategy_name: strat_b.name.clone(),
            branch_name: branch_b_name.clone(),
            worktree_path: worktree_b_path.clone(),
            status: status_b.clone(),
            output_text: res_b.unwrap_or_else(|e| format!("Execution error: {}", e)),
            lines_added: lines_add_b,
            lines_removed: lines_del_b,
            verification_passed: passed_b,
            execution_duration_ms: duration_b,
            diff: diff_b.clone(),
        };

        let has_changes_a = !diff_a.trim().is_empty();
        let has_changes_b = !diff_b.trim().is_empty();

        // 6. Arbitration Phase: Winner Arbitration & Auto-Merge
        let decision = if passed_a && !passed_b && has_changes_a {
            let merge_out = git_merge_branch_in_dir(&branch_a_name, &self.target_branch, Some(&self.repo_root))
                .unwrap_or_else(|e| format!("Auto-merge warning: {}", e));
            ArbitrationDecision::AutoMerged {
                winner_id: strat_a.id.clone(),
                branch_name: branch_a_name.clone(),
                merge_output: merge_out,
            }
        } else if passed_b && !passed_a && has_changes_b {
            let merge_out = git_merge_branch_in_dir(&branch_b_name, &self.target_branch, Some(&self.repo_root))
                .unwrap_or_else(|e| format!("Auto-merge warning: {}", e));
            ArbitrationDecision::AutoMerged {
                winner_id: strat_b.id.clone(),
                branch_name: branch_b_name.clone(),
                merge_output: merge_out,
            }
        } else if passed_a && passed_b && (has_changes_a || has_changes_b) {
            let rec = if has_changes_a && has_changes_b {
                let total_a = lines_add_a + lines_del_a;
                let total_b = lines_add_b + lines_del_b;
                if total_a < total_b || (total_a == total_b && duration_a <= duration_b) {
                    Some(strat_a.id.clone())
                } else {
                    Some(strat_b.id.clone())
                }
            } else if has_changes_a {
                Some(strat_a.id.clone())
            } else {
                Some(strat_b.id.clone())
            };

            ArbitrationDecision::BothPassedSplitDiff {
                recommended_winner: rec,
                diff_a: diff_a.clone(),
                diff_b: diff_b.clone(),
            }
        } else if !passed_a && !passed_b {
            ArbitrationDecision::BothFailed {
                error: format!(
                    "Both speculative branches failed verification.\n\nStrategy A ({}):\n{}\n\nStrategy B ({}):\n{}",
                    strat_a.name, err_a, strat_b.name, err_b
                ),
            }
        } else {
            ArbitrationDecision::NoChanges
        };

        // 7. Teardown Phase: Cleanup Disposable Ghost Worktrees
        let _ = git_worktree_remove_path(&worktree_a_path, Some(&branch_a_name), true, Some(&self.repo_root));
        let _ = git_worktree_remove_path(&worktree_b_path, Some(&branch_b_name), true, Some(&self.repo_root));

        let summary = Self::generate_summary_markdown(
            goal,
            &branch_res_a,
            &branch_res_b,
            &decision,
        );

        Ok(SpeculativeRaceResult {
            goal: goal.to_string(),
            results: vec![branch_res_a, branch_res_b],
            decision,
            summary,
        })
    }

    pub async fn run_ghost_race_with_session<F, Fut>(
        &self,
        goal: &str,
        model_cfg: &ModelConfig,
        crews: Vec<SpeculativeStrategy>,
        session_tree: &mut SessionTree,
        runner: F,
    ) -> Result<SpeculativeRaceResult>
    where
        F: Fn(PathBuf, SpeculativeStrategy, String, ModelConfig) -> Fut + Send + Sync + Clone + 'static,
        Fut: Future<Output = Result<String>> + Send + 'static,
    {
        if crews.len() < 2 {
            return Err(anyhow!("Ghost race requires at least 2 crews"));
        }

        let tau_dir = self.repo_root.join(".tau");
        let worktrees_parent = tau_dir.join("worktrees");
        std::fs::create_dir_all(&worktrees_parent)?;

        let tau_gitignore = tau_dir.join(".gitignore");
        if !tau_gitignore.exists() {
            let _ = std::fs::write(&tau_gitignore, "*\n!.gitignore\n");
        }

        let mut branch_infos: Vec<(SpeculativeStrategy, String, PathBuf)> = Vec::with_capacity(crews.len());
        for strat in &crews {
            let branch_name = format!("tau-ghost-{}", strat.id);
            let worktree_path = worktrees_parent.join(&strat.id);
            let _ = git_worktree_remove_path(&worktree_path, Some(&branch_name), true, Some(&self.repo_root));
            git_worktree_create_at(&self.base_branch, &branch_name, &worktree_path, Some(&self.repo_root))?;
            branch_infos.push((strat.clone(), branch_name, worktree_path));
        }

        let verify_cmd_opt = self.verification_command.clone();
        let mut race_futures = Vec::with_capacity(crews.len());
        for (strat, _, wt_path) in &branch_infos {
            let runner = runner.clone();
            let strat = strat.clone();
            let goal = goal.to_string();
            let wt_path = wt_path.clone();
            let cfg = model_cfg.clone();
            race_futures.push(async move {
                let start = Instant::now();
                let res = runner(wt_path, strat.clone(), goal, cfg).await;
                (res, strat, start.elapsed().as_millis() as u64)
            });
        }

        let race_results = futures_util::future::join_all(race_futures).await;
        let mut crew_results = Vec::with_capacity(crews.len());
        let mut arbitration_results = Vec::with_capacity(crews.len());

        for ((res, strat, duration), (_, branch_name, worktree_path)) in
            race_results.into_iter().zip(branch_infos.iter())
        {
            let ver = Self::verify_workspace(worktree_path.clone(), verify_cmd_opt.clone()).await;
            let diff = Self::get_branch_diff(worktree_path, &self.base_branch);
            let (lines_added, lines_removed) = Self::count_diff_lines(&diff);

            let _status = match &res {
                Ok(_) if ver.passed => SpeculativeStatus::Passed,
                Ok(_) => SpeculativeStatus::VerificationFailed { error: ver.output.clone() },
                Err(e) => SpeculativeStatus::Failed(e.to_string()),
            };

            let passed = res.is_ok() && ver.passed;
            let reward = if passed { 1.0 } else { 0.0 };
            let crew_node_id = session_tree.append_child_with_metadata(
                Role::Assistant,
                format!("Ghost crew {} completed goal: {}", strat.id, goal),
                Some(format!("spec-crew-{}", strat.id)),
                Some("speculative_race".to_string()),
                Some(serde_json::json!({
                    "strategyId": strat.id,
                    "branchName": branch_name,
                    "passed": passed,
                    "reward": reward,
                    "executionDurationMs": duration,
                })),
            );

            crew_results.push(SpeculativeCrewResult {
                crew_id: strat.id.clone(),
                strategy_id: strat.id.clone(),
                strategy_name: strat.name.clone(),
                branch_name: branch_name.clone(),
                worktree_path: worktree_path.clone(),
                passed,
                reward,
                execution_duration_ms: duration,
                verification_output: ver.output.clone(),
                diff: diff.clone(),
                lines_added,
                lines_removed,
                session_node_id: Some(crew_node_id),
            });

            arbitration_results.push((strat.id.clone(), strat.name.clone(), passed, duration, diff, ver.output));
        }

        let decision = Self::arbitrate_n_crews(&crew_results);
        let summary = Self::generate_ghost_race_summary(goal, &crew_results, &decision);

        for (_, branch_name, worktree_path) in &branch_infos {
            let _ = git_worktree_remove_path(worktree_path, Some(branch_name), true, Some(&self.repo_root));
        }

        let mut results = Vec::with_capacity(crew_results.len());
        for crew in crew_results {
            results.push(SpeculativeBranchResult {
                strategy_id: crew.strategy_id,
                strategy_name: crew.strategy_name,
                branch_name: crew.branch_name,
                worktree_path: crew.worktree_path,
                status: if crew.passed { SpeculativeStatus::Passed } else { SpeculativeStatus::Failed(crew.verification_output.clone()) },
                output_text: if crew.passed { "Crew passed verification".to_string() } else { crew.verification_output.clone() },
                lines_added: crew.lines_added,
                lines_removed: crew.lines_removed,
                verification_passed: crew.passed,
                execution_duration_ms: crew.execution_duration_ms,
                diff: crew.diff,
            });
        }

        Ok(SpeculativeRaceResult {
            goal: goal.to_string(),
            results,
            decision,
            summary,
        })
    }

    pub fn summarize_mcts_rewards(
        session_tree: &SessionTree,
        branch_head: Option<&str>,
    ) -> MctsRewardSummary {
        let head = branch_head.unwrap_or(&session_tree.active_node_id);
        let history = session_tree.get_branch_history(head);
        let mut rewards: Vec<(String, f64)> = Vec::new();
        let mut total_simulations = 0usize;
        let mut best_crew_id = String::new();
        let mut best_reward = f64::MIN;

        for node in history {
            if let Some(tcalls) = &node.tool_calls
                && let Some(arr) = tcalls.as_array()
            {
                for tc in arr {
                    if tc.get("type").and_then(|v| v.as_str()) == Some("function")
                        && let Some(name) = tc.get("function").and_then(|v| v.as_str())
                        && name == "speculative_race"
                    {
                        total_simulations += 1;
                        let reward = tc.get("reward").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let crew_id = tc.get("strategyId").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                        rewards.push((crew_id.clone(), reward));
                        if reward > best_reward {
                            best_reward = reward;
                            best_crew_id = crew_id.clone();
                        }
                    }
                }
            }
        }

        if best_crew_id.is_empty() {
            best_crew_id = head.to_string();
            best_reward = 0.0;
        }

        MctsRewardSummary { total_simulations, best_crew_id, best_reward, rewards }
    }

    async fn verify_workspace(worktree_path: PathBuf, verify_cmd: Option<String>) -> VerificationResult {
        let (cmd_bin, args) = if let Some(ref cmd_str) = verify_cmd {
            let parts: Vec<String> = cmd_str.split_whitespace().map(|s| s.to_string()).collect();
            if parts.is_empty() {
                ("cargo".to_string(), vec!["check".to_string(), "--workspace".to_string(), "--all-targets".to_string()])
            } else {
                (parts[0].clone(), parts[1..].to_vec())
            }
        } else if worktree_path.join("Cargo.toml").exists() {
            ("cargo".to_string(), vec!["check".to_string(), "--workspace".to_string(), "--all-targets".to_string()])
        } else if worktree_path.join("package.json").exists() {
            ("npm".to_string(), vec!["test".to_string()])
        } else {
            ("git".to_string(), vec!["status".to_string(), "--short".to_string()])
        };

        let mut command = tokio::process::Command::new(&cmd_bin);
        command.args(&args).current_dir(&worktree_path);
        command.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());

        let res = tokio::time::timeout(std::time::Duration::from_secs(120), async {
            let child = command.spawn()?;
            let output = child.wait_with_output().await?;
            Ok::<_, std::io::Error>(output)
        })
        .await;

        match res {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let combined = if stderr.is_empty() {
                    stdout.to_string()
                } else if stdout.is_empty() {
                    stderr.to_string()
                } else {
                    format!("{}\n{}", stdout, stderr)
                };
                VerificationResult {
                    passed: output.status.success(),
                    output: combined.trim().to_string(),
                }
            }
            Ok(Err(e)) => VerificationResult {
                passed: false,
                output: format!("Failed to spawn verification command '{}': {}", cmd_bin, e),
            },
            Err(_) => VerificationResult {
                passed: false,
                output: format!("Verification command '{}' timed out after 120s", cmd_bin),
            },
        }
    }

    fn get_branch_diff(worktree_path: &Path, base_branch: &str) -> String {
        let base = if base_branch.trim().is_empty() {
            "HEAD"
        } else {
            base_branch.trim()
        };

        // Try diff vs base branch
        let mut cmd = pi_tools::git::git_cmd();
        cmd.current_dir(worktree_path).args(["diff", base]);
        if let Ok(output) = cmd.output()
            && output.status.success()
        {
            let s = String::from_utf8_lossy(&output.stdout).to_string();
            if !s.trim().is_empty() {
                return s;
            }
        }

        // Fallback to diff HEAD
        let mut cmd_head = pi_tools::git::git_cmd();
        cmd_head.current_dir(worktree_path).args(["diff", "HEAD"]);
        if let Ok(output) = cmd_head.output()
            && output.status.success()
        {
            return String::from_utf8_lossy(&output.stdout).to_string();
        }

        String::new()
    }

    pub fn count_diff_lines(diff: &str) -> (usize, usize) {
        let mut added = 0;
        let mut removed = 0;
        for line in diff.lines() {
            if line.starts_with("+++") || line.starts_with("---") {
                continue;
            }
            if line.starts_with('+') {
                added += 1;
            } else if line.starts_with('-') {
                removed += 1;
            }
        }
        (added, removed)
    }

    fn generate_summary_markdown(
        goal: &str,
        res_a: &SpeculativeBranchResult,
        res_b: &SpeculativeBranchResult,
        decision: &ArbitrationDecision,
    ) -> String {
        let mut s = format!("### 🔮 Speculative Execution Race Report\n\n**Goal**: {}\n\n", goal);
        s.push_str("| Metric | Approach A | Approach B |\n");
        s.push_str("| :--- | :--- | :--- |\n");
        s.push_str(&format!(
            "| **Strategy** | {} | {} |\n",
            res_a.strategy_name, res_b.strategy_name
        ));
        s.push_str(&format!(
            "| **Status** | {} | {} |\n",
            if res_a.verification_passed { "✅ Passed" } else { "❌ Failed" },
            if res_b.verification_passed { "✅ Passed" } else { "❌ Failed" }
        ));
        s.push_str(&format!(
            "| **Duration** | {}ms | {}ms |\n",
            res_a.execution_duration_ms, res_b.execution_duration_ms
        ));
        s.push_str(&format!(
            "| **Diff** | +{} / -{} | +{} / -{} |\n\n",
            res_a.lines_added, res_a.lines_removed, res_b.lines_added, res_b.lines_removed
        ));

        match decision {
            ArbitrationDecision::AutoMerged { winner_id, branch_name, merge_output } => {
                s.push_str(&format!(
                    "🏆 **Decision**: Automatically merged winning branch `{}` (`{}`) into target.\n\n```\n{}\n```\n",
                    winner_id, branch_name, merge_output.trim()
                ));
            }
            ArbitrationDecision::BothPassedSplitDiff { recommended_winner, diff_a, diff_b } => {
                let rec_str = recommended_winner.as_deref().unwrap_or("None");
                s.push_str(&format!(
                    "⚖️ **Decision**: Both approaches succeeded verification! (Recommended: `{}`). Split-diff available for selection.\n\n",
                    rec_str
                ));
                s.push_str(&format!(
                    "<details><summary>Approach A Diff (+{}/-{})</summary>\n\n```diff\n{}\n```\n</details>\n\n",
                    res_a.lines_added, res_a.lines_removed, diff_a.trim()
                ));
                s.push_str(&format!(
                    "<details><summary>Approach B Diff (+{}/-{})</summary>\n\n```diff\n{}\n```\n</details>\n",
                    res_b.lines_added, res_b.lines_removed, diff_b.trim()
                ));
            }
            ArbitrationDecision::BothFailed { error } => {
                s.push_str(&format!(
                    "❌ **Decision**: Both speculative approaches failed verification.\n\n```\n{}\n```\n",
                    error
                ));
            }
            ArbitrationDecision::NoChanges => {
                s.push_str("ℹ️ **Decision**: No code modifications were generated by either branch.\n");
            }
        }

        s
    }

    fn arbitrate_n_crews(crews: &[SpeculativeCrewResult]) -> ArbitrationDecision {
        let passed: Vec<_> = crews.iter().filter(|c| c.passed).collect();
        if passed.is_empty() {
            return ArbitrationDecision::BothFailed { error: "All ghost crews failed verification".to_string() };
        }
        if passed.len() == 1 {
            let winner = passed[0];
            return ArbitrationDecision::AutoMerged {
                winner_id: winner.crew_id.clone(),
                branch_name: winner.branch_name.clone(),
                merge_output: format!(
                    "Winner: {} in {}ms",
                    winner.strategy_name, winner.execution_duration_ms
                ),
            };
        }
        let recommended = passed
            .iter()
            .min_by(|a, b| a.execution_duration_ms.cmp(&b.execution_duration_ms).then(a.lines_added.cmp(&b.lines_added)))
            .map(|c| c.crew_id.clone());
        ArbitrationDecision::BothPassedSplitDiff {
            recommended_winner: recommended,
            diff_a: passed.first().map(|c| c.diff.clone()).unwrap_or_default(),
            diff_b: passed.get(1).map(|c| c.diff.clone()).unwrap_or_default(),
        }
    }

    fn generate_ghost_race_summary(
        goal: &str,
        crews: &[SpeculativeCrewResult],
        decision: &ArbitrationDecision,
    ) -> String {
        let mut s = format!("### 👻 Ghost Crew Race Report\n\n**Goal**: {}\n\n", goal);
        s.push_str("| Crew | Strategy | Status | Reward | Duration | Diff |\n");
        s.push_str("| :--- | :--- | :--- | :--- | :--- | :--- |\n");
        for crew in crews {
            s.push_str(&format!(
                "| {} | {} | {} | {:.1} | {}ms | +{} / -{} |\n",
                crew.crew_id,
                crew.strategy_name,
                if crew.passed { "✅ Passed" } else { "❌ Failed" },
                crew.reward,
                crew.execution_duration_ms,
                crew.lines_added,
                crew.lines_removed,
            ));
        }
        match decision {
            ArbitrationDecision::AutoMerged { winner_id, merge_output, .. } => {
                s.push_str(&format!("\n🏆 **Decision**: Auto-merged crew `{}`. {}\n", winner_id, merge_output));
            }
            ArbitrationDecision::BothPassedSplitDiff { recommended_winner, .. } => {
                s.push_str(&format!("\n⚖️ **Decision**: Multiple crews passed. Recommended: `{}`.\n", recommended_winner.as_deref().unwrap_or("None")));
            }
            ArbitrationDecision::BothFailed { error } => {
                s.push_str(&format!("\n❌ **Decision**: All crews failed. {}\n", error));
            }
            ArbitrationDecision::NoChanges => s.push_str("\nℹ️ **Decision**: No changes produced.\n"),
        }
        s
    }

    pub fn create_tool_handler(self: &Arc<Self>, model_cfg: ModelConfig) -> Arc<dyn pi_tools::SpeculateToolHandler> {
        Arc::new(SpeculativeToolBridge {
            engine: self.clone(),
            model_cfg,
        })
    }

    pub fn init_global_handler(self: &Arc<Self>, model_cfg: ModelConfig) {
        let handler = self.create_tool_handler(model_cfg);
        pi_tools::register_speculate_handler(handler);
    }
}

pub struct SpeculativeToolBridge {
    pub engine: Arc<SpeculativeEngine>,
    pub model_cfg: ModelConfig,
}

impl pi_tools::SpeculateToolHandler for SpeculativeToolBridge {
    fn run_speculative_race<'a>(
        &'a self,
        args: &'a pi_tools::SpeculateArgs,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            let mut strategies = None;
            if args.strategy_a.is_some() || args.strategy_b.is_some() {
                strategies = Some(vec![
                    SpeculativeStrategy {
                        id: "spec-a".to_string(),
                        name: args.strategy_a.clone().unwrap_or_else(|| "Approach A".to_string()),
                        prompt_directive: args.strategy_a.clone().unwrap_or_default(),
                    },
                    SpeculativeStrategy {
                        id: "spec-b".to_string(),
                        name: args.strategy_b.clone().unwrap_or_else(|| "Approach B".to_string()),
                        prompt_directive: args.strategy_b.clone().unwrap_or_default(),
                    },
                ]);
            }

            let mut engine = (*self.engine).clone();
            if let Some(ref target) = args.target_branch {
                engine.target_branch = target.clone();
            }
            if let Some(ref vcmd) = args.verify_cmd {
                engine.verification_command = Some(vcmd.clone());
            }

            let res = engine
                .run_speculative_race(&args.goal, &self.model_cfg, strategies)
                .await?;
            Ok(res.summary)
        })
    }
}

impl SpeculativeToolBridge {
    pub async fn run_ghost_race<'a>(
        &'a self,
        args: &'a pi_tools::SpeculateArgs,
        session_tree: &'a mut SessionTree,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
        let engine = self.engine.clone();
        let cfg = self.model_cfg.clone();
        let strategies = args
            .strategy_a
            .as_ref()
            .or(args.strategy_b.as_ref())
            .map(|_| {
                vec![
                    SpeculativeStrategy {
                        id: "crew-grep-first".to_string(),
                        name: args.strategy_a.clone().unwrap_or_else(|| "Crew A: Grep-First".to_string()),
                        prompt_directive: args.strategy_a.clone().unwrap_or_default(),
                    },
                    SpeculativeStrategy {
                        id: "crew-read-first".to_string(),
                        name: args.strategy_b.clone().unwrap_or_else(|| "Crew B: Read-First".to_string()),
                        prompt_directive: args.strategy_b.clone().unwrap_or_default(),
                    },
                ]
            })
            .unwrap_or_else(SpeculativeStrategy::default_strategies);

        Box::pin(async move {
            let res = engine
                .run_ghost_race_with_session(&args.goal, &cfg, strategies, session_tree, |_wt_path, strat, goal, cfg| {
                    async move {
                        let mut agent = crate::AgentLoop::new(cfg);
                        let prompt = format!(
                            "Speculative Ghost Crew Goal: {}\n\nCrew Directive ({}):\n{}\n\nImplement the solution in this worktree and verify with cargo test / clippy when possible.",
                            goal, strat.name, strat.prompt_directive
                        );
                        let res = agent.run_turn(&prompt, |_| {}).await?;
                        Ok(res)
                    }
                })
                .await?;
            Ok(res.summary)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    fn setup_test_git_repo() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().to_path_buf();

        let _ = Command::new("git").arg("init").current_dir(&repo).output().unwrap();
        let _ = Command::new("git").args(["config", "user.name", "Test User"]).current_dir(&repo).output().unwrap();
        let _ = Command::new("git").args(["config", "user.email", "test@example.com"]).current_dir(&repo).output().unwrap();

        fs::write(repo.join("README.md"), "# Speculative Repo\n").unwrap();
        fs::write(repo.join("Cargo.toml"), "[package]\nname = \"spec-test\"\nversion = \"0.1.0\"\nedition = \"2021\"\n").unwrap();
        fs::create_dir_all(repo.join("src")).unwrap();
        fs::write(repo.join("src/lib.rs"), "pub fn add(a: i32, b: i32) -> i32 { a + b }\n").unwrap();

        let _ = Command::new("git").args(["add", "."]).current_dir(&repo).output().unwrap();
        let _ = Command::new("git").args(["commit", "-m", "Initial commit"]).current_dir(&repo).output().unwrap();

        (tmp, repo)
    }

    #[tokio::test]
    async fn test_speculative_race_auto_merge_single_winner() {
        let (_tmp, repo) = setup_test_git_repo();
        let engine = SpeculativeEngine::new(&repo)
            .with_verification_cmd("git status");

        let model_cfg = ModelConfig::resolve("mock/model");

        // Runner: Strategy A modifies src/lib.rs successfully, Strategy B errors
        let res = engine
            .run_speculative_race_with_runner(
                "Optimize arithmetic function",
                &model_cfg,
                None,
                |wt_path, strat, _goal, _cfg| async move {
                    if strat.id == "spec-a" {
                        fs::write(
                            wt_path.join("src/lib.rs"),
                            "pub fn add(a: i32, b: i32) -> i32 { (a as i64 + b as i64) as i32 }\n",
                        )?;
                        Ok("Approach A optimized successfully".to_string())
                    } else {
                        Err(anyhow!("Approach B failed to synthesize code"))
                    }
                },
            )
            .await
            .unwrap();

        assert_eq!(res.results.len(), 2);
        assert!(res.results[0].verification_passed);
        assert!(!res.results[1].verification_passed);

        match res.decision {
            ArbitrationDecision::AutoMerged { winner_id, .. } => {
                assert_eq!(winner_id, "spec-a");
            }
            other => panic!("Expected AutoMerged, got {:?}", other),
        }

        assert!(res.summary.contains("Speculative Execution Race Report"));
        assert!(res.summary.contains("Automatically merged winning branch"));

        // Worktrees must be cleaned up
        assert!(!repo.join(".tau/worktrees/spec-a").exists());
        assert!(!repo.join(".tau/worktrees/spec-b").exists());
    }

    #[tokio::test]
    async fn test_speculative_race_both_passed_split_diff() {
        let (_tmp, repo) = setup_test_git_repo();
        let engine = SpeculativeEngine::new(&repo)
            .with_verification_cmd("git status");

        let model_cfg = ModelConfig::resolve("mock/model");

        // Both strategies succeed with different changes
        let res = engine
            .run_speculative_race_with_runner(
                "Implement subtract function",
                &model_cfg,
                None,
                |wt_path, strat, _goal, _cfg| async move {
                    if strat.id == "spec-a" {
                        fs::write(
                            wt_path.join("src/lib.rs"),
                            "pub fn add(a: i32, b: i32) -> i32 { a + b }\npub fn sub(a: i32, b: i32) -> i32 { a - b }\n",
                        )?;
                    } else {
                        fs::write(
                            wt_path.join("src/lib.rs"),
                            "pub fn add(a: i32, b: i32) -> i32 { a + b }\n/// Subtracts b from a\npub fn sub(a: i32, b: i32) -> i32 {\n    let diff = a - b;\n    diff\n}\n",
                        )?;
                    }
                    Ok("Implementation complete".to_string())
                },
            )
            .await
            .unwrap();

        match res.decision {
            ArbitrationDecision::BothPassedSplitDiff { recommended_winner, diff_a, diff_b } => {
                assert_eq!(recommended_winner, Some("spec-a".to_string())); // spec-a is more concise
                assert!(!diff_a.is_empty());
                assert!(!diff_b.is_empty());
            }
            other => panic!("Expected BothPassedSplitDiff, got {:?}", other),
        }

        assert!(res.summary.contains("Both approaches succeeded verification!"));

        // Worktrees cleaned up
        assert!(!repo.join(".tau/worktrees/spec-a").exists());
        assert!(!repo.join(".tau/worktrees/spec-b").exists());
    }

    #[tokio::test]
    async fn test_speculative_race_both_failed() {
        let (_tmp, repo) = setup_test_git_repo();
        let engine = SpeculativeEngine::new(&repo)
            .with_verification_cmd("git status");

        let model_cfg = ModelConfig::resolve("mock/model");

        let res = engine
            .run_speculative_race_with_runner(
                "Impossible task",
                &model_cfg,
                None,
                |_wt_path, _strat, _goal, _cfg| async move {
                    Err(anyhow!("Synthesizer timeout error"))
                },
            )
            .await
            .unwrap();

        match res.decision {
            ArbitrationDecision::BothFailed { error } => {
                assert!(error.contains("failed"));
            }
            other => panic!("Expected BothFailed, got {:?}", other),
        }

        assert!(res.summary.contains("Both speculative approaches failed verification"));
    }

    #[test]
    fn test_count_diff_lines() {
        let diff = r#"
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,4 @@
 pub fn add(a: i32, b: i32) -> i32 {
-    a + b
+    let sum = a + b;
+    sum
 }
"#;
        let (added, removed) = SpeculativeEngine::count_diff_lines(diff);
        assert_eq!(added, 2);
        assert_eq!(removed, 1);
    }

    #[tokio::test]
    async fn test_ghost_race_two_crews_winner_picked() {
        let (_tmp, repo) = setup_test_git_repo();
        let engine = SpeculativeEngine::new(&repo).with_verification_cmd("git status");
        let model_cfg = ModelConfig::resolve("mock/model");
        let mut session_tree = SessionTree::new();

        let res = engine
            .run_ghost_race_with_session(
                "Add subtract helper",
                &model_cfg,
                vec![
                    SpeculativeStrategy { id: "crew-a".to_string(), name: "Crew A".to_string(), prompt_directive: "write concise code".to_string() },
                    SpeculativeStrategy { id: "crew-b".to_string(), name: "Crew B".to_string(), prompt_directive: "write verbose code".to_string() },
                ],
                &mut session_tree,
                |wt_path, strat, _goal, _cfg| async move {
                    if strat.id == "crew-a" {
                        fs::write(wt_path.join("src/lib.rs"), "pub fn add(a: i32, b: i32) -> i32 { a + b }\npub fn sub(a: i32, b: i32) -> i32 { a - b }\n")?;
                        Ok("crew-a done".to_string())
                    } else {
                        Err(anyhow!("crew-b timeout"))
                    }
                },
            )
            .await
            .unwrap();

        assert_eq!(res.results.len(), 2);
        assert!(matches!(res.decision, ArbitrationDecision::AutoMerged { winner_id, .. } if winner_id == "crew-a"));
        assert!(res.summary.contains("Ghost Crew Race Report"));
        assert_eq!(session_tree.node_count(), 3);
        assert!(!repo.join(".tau/worktrees/crew-a").exists());
        assert!(!repo.join(".tau/worktrees/crew-b").exists());
    }

    #[tokio::test]
    async fn test_mcts_reward_summary_from_session() {
        let mut tree = SessionTree::new();
        let _tool_node = tree.append_child_with_metadata(
            Role::Assistant,
            "Reward trace".to_string(),
            Some("spec-crew-crew-a".to_string()),
            Some("speculative_race".to_string()),
            Some(serde_json::json!([{"type":"function","function":"speculative_race","strategyId":"crew-a","reward":1.0}])),
        );
        let _fail_node = tree.append_child_with_metadata(
            Role::Assistant,
            "Failed trace".to_string(),
            Some("spec-crew-crew-b".to_string()),
            Some("speculative_race".to_string()),
            Some(serde_json::json!([{"type":"function","function":"speculative_race","strategyId":"crew-b","reward":0.0}])),
        );

        let summary = SpeculativeEngine::summarize_mcts_rewards(&tree, None);
        assert_eq!(summary.total_simulations, 2);
        assert_eq!(summary.best_crew_id, "crew-a");
        assert_eq!(summary.best_reward, 1.0);
        assert_eq!(summary.rewards.len(), 2);
    }
}
