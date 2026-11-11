use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::agent::{AgentKind, MetafugeAgent};
use crate::manifest::Manifest;
use crate::support;
use crate::verdict::Verdict;

pub const RUN_SCHEMA: u32 = 1;

/// The typed state machine. Every transition writes the whole state to
/// `<out>/.metafuge/run.json` before the next stage starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Spec,
    Scaffold,
    Forge,
    Destroy,
    Crucible,
    Director,
    Seal,
    Done,
}

impl Stage {
    pub const ORDER: [Stage; 7] = [
        Stage::Spec,
        Stage::Scaffold,
        Stage::Forge,
        Stage::Destroy,
        Stage::Crucible,
        Stage::Director,
        Stage::Seal,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Stage::Spec => "spec",
            Stage::Scaffold => "scaffold",
            Stage::Forge => "forge",
            Stage::Destroy => "destroy",
            Stage::Crucible => "crucible",
            Stage::Director => "director",
            Stage::Seal => "seal",
            Stage::Done => "done",
        }
    }

    pub const fn following(self) -> Stage {
        match self {
            Stage::Spec => Stage::Scaffold,
            Stage::Scaffold => Stage::Forge,
            Stage::Forge => Stage::Destroy,
            Stage::Destroy => Stage::Crucible,
            Stage::Crucible => Stage::Director,
            Stage::Director => Stage::Seal,
            Stage::Seal | Stage::Done => Stage::Done,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskOwner {
    Forger,
    Destroyer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    InFlight,
    Complete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskState {
    pub id: String,
    pub owner: TaskOwner,
    pub status: TaskStatus,
    pub attempts: u32,
    /// Trimmed diagnostics for the next attempt only.
    pub diagnostic: Option<String>,
}

/// Which agent the run used, persisted so `resume` does not need to be told.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    pub kind: AgentKind,
    pub command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunState {
    pub schema: u32,
    pub run_id: String,
    pub prompt: String,
    pub stack: String,
    pub out_dir: PathBuf,
    pub stage: Stage,
    pub status: RunStatus,
    pub round: u32,
    pub max_rounds: u32,
    pub seal_requested: bool,
    pub agent: AgentState,
    pub tasks: Vec<TaskState>,
    pub manifest: Option<Manifest>,
    pub verdict: Option<Verdict>,
    pub telemetry: Vec<PathBuf>,
    pub sealed_commit: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl RunState {
    pub fn record_path(out: &Path) -> PathBuf {
        out.join(".metafuge").join("run.json")
    }

    pub fn load(out: &Path) -> Result<RunState> {
        let path = Self::record_path(out);
        let raw = std::fs::read_to_string(&path).with_context(|| {
            format!(
                "no run record at `{}`; start one with `metafuge forge`",
                path.display()
            )
        })?;
        let state: RunState = serde_json::from_str(&raw)
            .with_context(|| format!("`{}` is not a valid run record", path.display()))?;
        if state.schema != RUN_SCHEMA {
            bail!(
                "run record schema {} is not supported by this build (expected {RUN_SCHEMA})",
                state.schema
            );
        }
        Ok(state)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::record_path(&self.out_dir);
        let body = serde_json::to_string_pretty(self)?;
        support::write_atomic(&path, &body)
    }

    pub fn task(&self, id: &str) -> Option<&TaskState> {
        self.state_task(id)
    }

    fn state_task(&self, id: &str) -> Option<&TaskState> {
        self.tasks.iter().find(|task| task.id == id)
    }
}

/// A progress line emitted by a stage. Never serialized as chat: it is a
/// one-line status record for the operator.
#[derive(Debug, Clone)]
pub struct Progress {
    pub stage: Stage,
    pub round: u32,
    pub message: String,
}

/// Operator-facing progress sink. The engine owns the only implementation.
pub type ProgressSink = Box<dyn FnMut(&Progress)>;

/// A live run: persisted state plus the agent that does the work.
pub struct Run {
    pub state: RunState,
    pub agent: Arc<dyn MetafugeAgent>,
    progress: Option<ProgressSink>,
}

impl Run {
    pub fn new(state: RunState, agent: Arc<dyn MetafugeAgent>) -> Self {
        Self {
            state,
            agent,
            progress: None,
        }
    }

    pub fn with_progress(mut self, progress: ProgressSink) -> Self {
        self.progress = Some(progress);
        self
    }

    pub fn report(&mut self, message: impl Into<String>) {
        let stage = self.state.stage;
        let round = self.state.round;
        if let Some(callback) = self.progress.as_mut() {
            callback(&Progress {
                stage,
                round,
                message: message.into(),
            });
        }
    }

    pub fn metafuge_dir(&self) -> PathBuf {
        self.state.out_dir.join(".metafuge")
    }

    pub fn telemetry_dir(&self) -> PathBuf {
        self.metafuge_dir().join("telemetry")
    }

    pub fn manifest(&self) -> Result<&Manifest> {
        self.state
            .manifest
            .as_ref()
            .context("run has no manifest; the spec stage has not produced one")
    }

    pub fn save(&mut self) -> Result<()> {
        self.state.updated_at = support::now();
        self.state.save()
    }

    /// Register every task in the manifest without disturbing prior progress.
    pub fn ensure_tasks(&mut self, manifest: &Manifest) {
        for task in &manifest.forger_tasks {
            self.push_task(&task.id, TaskOwner::Forger);
        }
        for task in &manifest.destroyer_tasks {
            self.push_task(&task.id, TaskOwner::Destroyer);
        }
    }

    fn push_task(&mut self, id: &str, owner: TaskOwner) {
        if self.state.tasks.iter().any(|task| task.id == id) {
            return;
        }
        self.state.tasks.push(TaskState {
            id: id.to_string(),
            owner,
            status: TaskStatus::Pending,
            attempts: 0,
            diagnostic: None,
        });
    }

    pub fn task_status(&self, id: &str) -> Option<TaskStatus> {
        self.state.task(id).map(|task| task.status)
    }

    pub fn set_task_status(&mut self, id: &str, status: TaskStatus) -> Result<()> {
        let task = self
            .state
            .tasks
            .iter_mut()
            .find(|task| task.id == id)
            .with_context(|| format!("task `{id}` is not registered for this run"))?;
        task.status = status;
        if status == TaskStatus::InFlight {
            task.attempts += 1;
        }
        Ok(())
    }

    pub fn take_diagnostic(&mut self, id: &str) -> Option<String> {
        self.state
            .tasks
            .iter_mut()
            .find(|task| task.id == id)
            .and_then(|task| task.diagnostic.take())
    }

    /// Queue a surgical repair: only these tasks go back to pending, and they
    /// carry only the trimmed diagnostic from the failed round.
    pub fn queue_repair(&mut self, ids: &[String], diagnostic: &str) -> Result<()> {
        for id in ids {
            let task = self
                .state
                .tasks
                .iter_mut()
                .find(|task| task.id == *id)
                .with_context(|| format!("task `{id}` is not registered for this run"))?;
            task.status = TaskStatus::Pending;
            task.diagnostic = Some(diagnostic.to_string());
        }
        Ok(())
    }

    pub fn record_telemetry(&mut self, path: PathBuf) {
        if !self.state.telemetry.contains(&path) {
            self.state.telemetry.push(path);
        }
    }

    pub fn pending_tasks(&self) -> Vec<String> {
        self.state
            .tasks
            .iter()
            .filter(|task| task.status != TaskStatus::Complete)
            .map(|task| task.id.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stages_follow_the_declared_order() {
        let mut stage = Stage::Spec;
        for expected in &Stage::ORDER[1..] {
            stage = stage.following();
            assert_eq!(stage, *expected);
        }
        assert_eq!(Stage::Seal.following(), Stage::Done);
    }

    #[test]
    fn run_state_round_trips_through_disk() {
        let dir = tempfile::tempdir().expect("temp dir");
        let state = crate::testsupport::state(dir.path());
        state.save().expect("save");
        let loaded = RunState::load(dir.path()).expect("load");
        assert_eq!(loaded.run_id, state.run_id);
        assert_eq!(loaded.stage, Stage::Spec);
        assert_eq!(loaded.status, RunStatus::Running);
    }

    #[test]
    fn run_state_rejects_a_foreign_schema() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut state = crate::testsupport::state(dir.path());
        state.schema = 99;
        state.save().expect("save");
        let error = RunState::load(dir.path()).expect_err("schema must be rejected");
        assert!(error.to_string().contains("schema 99"));
    }

    #[test]
    fn queue_repair_only_touches_named_tasks() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut run = crate::testsupport::run(dir.path());
        let manifest = crate::testsupport::manifest();
        run.ensure_tasks(&manifest);
        run.queue_repair(&["f2".to_string()], "error[E0308]")
            .expect("queue");
        assert_eq!(run.task_status("f2"), Some(TaskStatus::Pending));
        assert_eq!(run.task_status("f1"), Some(TaskStatus::Pending));
        assert_eq!(run.take_diagnostic("f2").as_deref(), Some("error[E0308]"));
        assert!(run.take_diagnostic("f1").is_none());
    }
}
