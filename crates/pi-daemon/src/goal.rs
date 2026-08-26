//! Autonomous goal mode: persisted goal jobs run to completion by an agent
//! loop, gated by turn/token budgets and a verification gate.
//!
//! Goals persist as JSON at `<data_dir>/goals/<id>.json` so the daemon can
//! resume tracking across restarts.

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum GoalStatus {
    Pending,
    Running { turns: u32, tokens_used: u64 },
    Succeeded,
    Failed { reason: String },
    HaltedBudget,
    HaltedUnverified,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalJob {
    pub id: String,
    pub goal: String,
    pub max_turns: u32,
    pub max_total_tokens: u64,
    pub verify_prompt: String,
    pub status: GoalStatus,
    pub created_at: String,
    pub updated_at: String,
}

impl GoalJob {
    pub fn new(goal: &str, max_turns: u32, max_total_tokens: u64, verify_prompt: &str) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            id: format!(
                "goal-{}-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0),
                GOAL_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ),
            goal: goal.to_string(),
            max_turns,
            max_total_tokens,
            verify_prompt: verify_prompt.to_string(),
            status: GoalStatus::Pending,
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

/// What a worker turn returns. The daemon wires this to AgentLoop; tests use
/// scripted implementations.
#[async_trait::async_trait]
pub trait GoalWorker: Send + Sync {
    /// Run one work turn; returns (output summary, tokens used).
    async fn work_turn(&self, prompt: &str) -> Result<(String, u64)>;
}

struct VerifyAnswer {
    complete: bool,
    #[allow(dead_code)]
    reason: String,
}

/// Parse the verifier's lenient JSON answer. Unparseable => not complete.
fn parse_verify(raw: &str) -> VerifyAnswer {
    let trimmed = raw.trim();
    let json_start = trimmed.find('{');
    if let Some(start) = json_start {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&trimmed[start..]) {
            return VerifyAnswer {
                complete: v.get("complete").and_then(|c| c.as_bool()).unwrap_or(false),
                reason: v
                    .get("reason")
                    .and_then(|r| r.as_str())
                    .unwrap_or_default()
                    .to_string(),
            };
        }
    }
    // Fallback heuristic: explicit "complete": true anywhere in the text.
    let lower = trimmed.to_lowercase();
    VerifyAnswer {
        complete: lower.contains("\"complete\": true") || lower.contains("\"complete\":true"),
        reason: String::new(),
    }
}

pub struct GoalRunner {
    goals_dir: PathBuf,
}

static GOAL_SEQ: AtomicUsize = AtomicUsize::new(1);

impl GoalRunner {
    pub fn new<P: AsRef<Path>>(data_dir: P) -> Result<Self> {
        let goals_dir = data_dir.as_ref().join("goals");
        std::fs::create_dir_all(&goals_dir)
            .with_context(|| format!("Failed to create goals dir: {:?}", goals_dir))?;
        Ok(Self { goals_dir })
    }

    fn path_for(&self, id: &str) -> PathBuf {
        self.goals_dir.join(format!("{id}.json"))
    }

    pub fn save(&self, job: &GoalJob) -> Result<()> {
        let path = self.path_for(&job.id);
        let tmp = path.with_extension("json.tmp");
        let json = serde_json::to_string_pretty(job)?;
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    pub fn load(&self, id: &str) -> Result<Option<GoalJob>> {
        let path = self.path_for(id);
        if !path.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&path)?;
        Ok(Some(serde_json::from_str(&text)?))
    }

    pub fn list(&self) -> Result<Vec<GoalJob>> {
        let mut jobs = Vec::new();
        for entry in std::fs::read_dir(&self.goals_dir)?.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    if let Ok(job) = serde_json::from_str::<GoalJob>(&text) {
                        jobs.push(job);
                    }
                }
            }
        }
        jobs.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        Ok(jobs)
    }

    /// Create and persist a new pending goal.
    pub fn create(&self, goal: &str, max_turns: u32, max_total_tokens: u64, verify_prompt: &str) -> Result<GoalJob> {
        let mut job = GoalJob::new(goal, max_turns, max_total_tokens, verify_prompt);
        job.status = GoalStatus::Pending;
        self.save(&job)?;
        Ok(job)
    }

    /// Run the autonomy loop for one goal until success, failure, or budget halt.
    pub async fn run(&self, mut job: GoalJob, worker: std::sync::Arc<dyn GoalWorker>) -> Result<GoalJob> {
        let mut turns: u32 = 0;
        let mut tokens_used: u64 = 0;
        let mut unverified_streak: u32 = 0;

        job.status = GoalStatus::Running { turns: 0, tokens_used: 0 };
        self.save(&job)?;

        loop {
            // Budget gates checked BEFORE each turn.
            if turns >= job.max_turns {
                job.status = GoalStatus::HaltedUnverified;
                job.updated_at = Utc::now().to_rfc3339();
                self.save(&job)?;
                return Ok(job);
            }
            if tokens_used >= job.max_total_tokens {
                job.status = GoalStatus::HaltedBudget;
                job.updated_at = Utc::now().to_rfc3339();
                self.save(&job)?;
                return Ok(job);
            }

            let progress = format!(
                "GOAL: {}\n\nProgress so far ({} turns done):\nWork toward the goal now.",
                job.goal, turns
            );
            let (_work_output, work_tokens) = match worker.work_turn(&progress).await {
                Ok(r) => r,
                Err(e) => {
                    job.status = GoalStatus::Failed { reason: e.to_string() };
                    job.updated_at = Utc::now().to_rfc3339();
                    self.save(&job)?;
                    return Ok(job);
                }
            };
            turns += 1;
            tokens_used += work_tokens;

            // Verification gate after each work turn.
            let verify_input = format!("{}\n\nWork transcript summary:\n{}", job.verify_prompt, _work_output);
            let verdict = match worker.work_turn(&verify_input).await {
                Ok((raw, vt)) => {
                    tokens_used += vt;
                    parse_verify(&raw)
                }
                Err(e) => {
                    job.status = GoalStatus::Failed { reason: e.to_string() };
                    job.updated_at = Utc::now().to_rfc3339();
                    self.save(&job)?;
                    return Ok(job);
                }
            };

            if verdict.complete {
                job.status = GoalStatus::Succeeded;
                job.updated_at = Utc::now().to_rfc3339();
                self.save(&job)?;
                return Ok(job);
            }

            unverified_streak += 1;
            if turns >= 3 && unverified_streak >= 2 {
                job.status = GoalStatus::HaltedUnverified;
                job.updated_at = Utc::now().to_rfc3339();
                self.save(&job)?;
                return Ok(job);
            }

            job.status = GoalStatus::Running { turns, tokens_used };
            job.updated_at = Utc::now().to_rfc3339();
            self.save(&job)?;
        }
    }

    pub fn next_seq() -> usize {
        GOAL_SEQ.fetch_add(1, Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ScriptedWorker {
        answers: parking_lot::Mutex<VecDeque<(String, u64)>>,
    }
    use std::collections::VecDeque;

    #[async_trait::async_trait]
    impl GoalWorker for ScriptedWorker {
        async fn work_turn(&self, prompt: &str) -> Result<(String, u64)> {
            // Verification turns (non-GOAL prompts) consume scripted verdicts;
            // work turns return a canned progress line.
            if !prompt.starts_with("GOAL:") {
                let mut q = self.answers.lock();
                if q.is_empty() {
                    return Ok(("{\"complete\": false}".to_string(), 10));
                }
                return Ok(q.pop_front().unwrap());
            }
            let _ = self.answers.lock();
            Ok(("work step done".to_string(), 10))
        }
    }

    fn worker(steps: Vec<(&str, u64)>) -> std::sync::Arc<dyn GoalWorker> {
        std::sync::Arc::new(ScriptedWorker {
            answers: parking_lot::Mutex::new(
                steps.into_iter().map(|(s, t)| (s.to_string(), t)).collect(),
            ),
        })
    }

    #[tokio::test]
    async fn test_success_path() {
        let tmp = tempfile::tempdir().unwrap();
        let runner = GoalRunner::new(tmp.path()).unwrap();
        let job = runner
            .create("Do the thing", 10, 100_000, "Is it complete?")
            .unwrap();
        let w = worker(vec![
            ("{\"complete\": true, \"reason\": \"done\"}", 50),
        ]);
        let result = runner.run(job, w).await.unwrap();
        assert_eq!(result.status, GoalStatus::Succeeded);
        // Persistence round-trip.
        let loaded = runner.load(&result.id).unwrap().unwrap();
        assert_eq!(loaded.status, GoalStatus::Succeeded);
    }

    #[tokio::test]
    async fn test_budget_halt_fires() {
        let tmp = tempfile::tempdir().unwrap();
        let runner = GoalRunner::new(tmp.path()).unwrap();
        let job = runner.create("Burn tokens", 10, 60, "Done?").unwrap();
        let w = worker(vec![("step", 100)]);
        let result = runner.run(job, w).await.unwrap();
        assert_eq!(result.status, GoalStatus::HaltedBudget);
    }

    #[tokio::test]
    async fn test_turn_limit_halts_unverified() {
        let tmp = tempfile::tempdir().unwrap();
        let runner = GoalRunner::new(tmp.path()).unwrap();
        let job = runner.create("Never finishes", 2, 1_000_000, "Done?").unwrap();
        let w = worker(vec![
            ("{\"complete\": false}", 10),
            ("{\"complete\": false}", 10),
        ]);
        let result = runner.run(job, w).await.unwrap();
        assert_eq!(result.status, GoalStatus::HaltedUnverified);
    }

    #[test]
    fn test_parse_verify_lenient() {
        assert!(parse_verify("{\"complete\": true}").complete);
        assert!(parse_verify("noise {\"complete\": true, \"reason\": \"ok\"} tail").complete);
        assert!(!parse_verify("{\"complete\": false}").complete);
        assert!(!parse_verify("garbage").complete);
    }

    #[test]
    fn test_goal_persistence_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let runner = GoalRunner::new(tmp.path()).unwrap();
        let job = runner.create("g", 5, 500, "v?").unwrap();
        let reloaded = GoalRunner::new(tmp.path()).unwrap();
        let loaded = reloaded.load(&job.id).unwrap().unwrap();
        assert_eq!(loaded.goal, "g");
        assert_eq!(reloaded.list().unwrap().len(), 1);
    }
}
