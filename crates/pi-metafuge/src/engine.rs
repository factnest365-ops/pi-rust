use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use serde::Serialize;
use uuid::Uuid;

use crate::agent::{AgentDescription, AgentKind, MetafugeAgent, SubprocessAgent, TemplateAgent};
use crate::run::{
    AgentState, Progress, RUN_SCHEMA, Run, RunState, RunStatus, Stage, TaskState, TaskStatus,
};
use crate::stages::{self, StageOutcome};
use crate::support;
use crate::verdict::Verdict;

/// Schema for the JSON report printed by `--json`.
pub const REPORT_SCHEMA: u32 = 1;
/// Repair budget used when `--max-rounds` is not given.
pub const DEFAULT_MAX_ROUNDS: u32 = 3;
/// The only stack this build can forge. There is no fallback.
pub const SUPPORTED_STACK: &str = "rust";

/// Everything `metafuge forge` needs, validated before a single byte is written.
#[derive(Debug, Clone)]
pub struct ForgeOptions {
    pub prompt: String,
    pub out_dir: PathBuf,
    pub stack: String,
    pub max_rounds: u32,
    pub agent: AgentKind,
    pub agent_cmd: Option<String>,
    pub seal: bool,
}

impl ForgeOptions {
    pub fn validate(&self) -> Result<()> {
        if self.stack != SUPPORTED_STACK {
            bail!(
                "unsupported stack `{}`: this build forges `{SUPPORTED_STACK}` only, with no fallback",
                self.stack
            );
        }
        if self.prompt.trim().is_empty() {
            bail!("the prompt is empty; state the idea once");
        }
        if self.max_rounds == 0 {
            bail!("--max-rounds must be at least 1");
        }
        match self.agent {
            AgentKind::Subprocess => {
                let command = self.agent_cmd.as_deref().map(str::trim).unwrap_or_default();
                if command.is_empty() {
                    bail!("--agent subprocess requires --agent-cmd <command>");
                }
            }
            AgentKind::Template => {
                if self.agent_cmd.is_some() {
                    bail!("--agent-cmd is only meaningful with --agent subprocess");
                }
            }
        }
        let record = RunState::record_path(&self.out_dir);
        if record.exists() {
            bail!(
                "a run record already exists at `{}`; continue it with `metafuge resume --out {}` or forge into a fresh directory",
                record.display(),
                self.out_dir.display()
            );
        }
        if self.out_dir.exists() && !directory_is_empty(&self.out_dir)? {
            bail!(
                "out directory `{}` is not empty; forge writes into a new or empty directory",
                self.out_dir.display()
            );
        }
        Ok(())
    }

    pub fn build_agent(&self) -> Result<Arc<dyn MetafugeAgent>> {
        build_agent(self.agent, self.agent_cmd.as_deref())
    }

    /// The first persisted state. Nothing has happened yet, so every field that
    /// describes work is empty.
    pub fn initial_state(&self) -> RunState {
        let now = support::now();
        RunState {
            schema: RUN_SCHEMA,
            run_id: Uuid::new_v4().to_string(),
            prompt: self.prompt.clone(),
            stack: self.stack.clone(),
            out_dir: self.out_dir.clone(),
            stage: Stage::Spec,
            status: RunStatus::Running,
            round: 1,
            max_rounds: self.max_rounds,
            seal_requested: self.seal,
            agent: AgentState {
                kind: self.agent,
                command: self.agent_cmd.clone(),
            },
            tasks: Vec::new(),
            manifest: None,
            verdict: None,
            telemetry: Vec::new(),
            sealed_commit: None,
            error: None,
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

fn directory_is_empty(path: &Path) -> Result<bool> {
    let mut entries =
        std::fs::read_dir(path).with_context(|| format!("could not read `{}`", path.display()))?;
    Ok(entries.next().is_none())
}

/// Rebuild the agent a run was started with.
pub fn build_agent(kind: AgentKind, command: Option<&str>) -> Result<Arc<dyn MetafugeAgent>> {
    match kind {
        AgentKind::Template => Ok(Arc::new(TemplateAgent)),
        AgentKind::Subprocess => {
            let command = command
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .context("the recorded subprocess agent has no command; pass --agent-cmd")?;
            Ok(Arc::new(SubprocessAgent::new(command)))
        }
    }
}

/// Terminal description of a run, printed once in human or JSON form.
#[derive(Debug, Clone, Serialize)]
pub struct RunReport {
    pub schema: u32,
    pub ok: bool,
    pub run_id: String,
    pub out_dir: PathBuf,
    pub status: RunStatus,
    pub stage: Stage,
    pub rounds: u32,
    pub agent: AgentDescription,
    pub tasks: Vec<TaskState>,
    pub verdict: Option<Verdict>,
    pub telemetry_dir: PathBuf,
    pub sealed_commit: Option<String>,
}

impl RunReport {
    pub fn completed_tasks(&self) -> usize {
        self.tasks
            .iter()
            .filter(|task| task.status == TaskStatus::Complete)
            .count()
    }

    pub fn total_tasks(&self) -> usize {
        self.tasks.len()
    }
}

/// Drives the state machine. The engine owns the round cap; stages never do.
pub struct Engine {
    agent: Arc<dyn MetafugeAgent>,
    quiet: bool,
}

impl Engine {
    pub fn new(agent: Arc<dyn MetafugeAgent>) -> Self {
        Self {
            agent,
            quiet: false,
        }
    }

    /// Suppress progress lines (used by `--json`, where stdout must stay one object).
    pub fn quiet(mut self, quiet: bool) -> Self {
        self.quiet = quiet;
        self
    }

    pub async fn drive(&self, mut state: RunState) -> Result<RunReport> {
        if state.stack != SUPPORTED_STACK {
            bail!(
                "run `{}` was recorded for stack `{}`; this build only forges `{SUPPORTED_STACK}`",
                state.run_id,
                state.stack
            );
        }
        state.status = RunStatus::Running;
        state.error = None;
        let quiet = self.quiet;
        let mut run = Run::new(state, Arc::clone(&self.agent)).with_progress(Box::new(
            move |progress: &Progress| {
                if !quiet {
                    eprintln!(
                        "[{} r{}] {}",
                        progress.stage.as_str(),
                        progress.round,
                        progress.message
                    );
                }
            },
        ));
        run.save()?;
        run.report(format!("agent: {}", self.agent.describe().note));

        loop {
            let stage = run.state.stage;
            if stage == Stage::Done {
                run.state.status = RunStatus::Completed;
                run.save()?;
                break;
            }
            let outcome = match stages::dispatch(stage, &mut run).await {
                Ok(outcome) => outcome,
                Err(error) => {
                    run.state.status = RunStatus::Failed;
                    run.state.error = Some(format!("{error:#}"));
                    let _ = run.save();
                    return Err(error.context(format!("stage `{}` failed", stage.as_str())));
                }
            };
            match outcome {
                StageOutcome::Advance(next) => {
                    run.state.stage = next;
                    run.save()?;
                }
                StageOutcome::Retry => {
                    if run.state.round >= run.state.max_rounds {
                        let note = format!(
                            "repair round cap reached: {} round(s) exhausted without a clean crucible and an approved director verdict",
                            run.state.max_rounds
                        );
                        run.state.status = RunStatus::Failed;
                        run.state.error = Some(note.clone());
                        run.save()?;
                        let telemetry = ensure_telemetry_dir(&run);
                        bail!(
                            "{note}\nfinal telemetry directory:\n{}",
                            telemetry.display()
                        );
                    }
                    run.state.round += 1;
                    run.state.stage = Stage::Forge;
                    run.save()?;
                    run.report(format!(
                        "repair round {} of {} (forge, then crucible)",
                        run.state.round, run.state.max_rounds
                    ));
                }
                StageOutcome::Complete => {
                    run.state.stage = Stage::Done;
                    run.state.status = RunStatus::Completed;
                    run.save()?;
                    break;
                }
                StageOutcome::Fail(message) => {
                    run.state.status = RunStatus::Failed;
                    run.state.error = Some(message.clone());
                    run.save()?;
                    let telemetry = ensure_telemetry_dir(&run);
                    bail!(
                        "{message}\nfinal telemetry directory:\n{}",
                        telemetry.display()
                    );
                }
            }
        }

        let report = RunReport {
            schema: REPORT_SCHEMA,
            ok: run.state.status == RunStatus::Completed,
            run_id: run.state.run_id.clone(),
            out_dir: run.state.out_dir.clone(),
            status: run.state.status,
            stage: run.state.stage,
            rounds: run.state.round,
            agent: self.agent.describe(),
            tasks: run.state.tasks.clone(),
            verdict: run.state.verdict.clone(),
            telemetry_dir: run.telemetry_dir(),
            sealed_commit: run.state.sealed_commit.clone(),
        };
        run.report(format!(
            "run complete in {} round(s): {}",
            report.rounds,
            report.status_line()
        ));
        Ok(report)
    }
}

impl RunReport {
    fn status_line(&self) -> &'static str {
        match self.status {
            RunStatus::Running => "incomplete",
            RunStatus::Completed => "completed",
            RunStatus::Failed => "failed",
        }
    }
}

fn ensure_telemetry_dir(run: &Run) -> PathBuf {
    let directory = run.telemetry_dir();
    if let Err(error) = std::fs::create_dir_all(&directory) {
        eprintln!(
            "metafuge: warning: could not create telemetry directory `{}`: {error}",
            directory.display()
        );
    }
    directory
}

/// Forge a new repository from one prompt.
pub async fn forge(options: ForgeOptions, quiet: bool) -> Result<RunReport> {
    options.validate()?;
    let agent = options.build_agent()?;
    Engine::new(agent)
        .quiet(quiet)
        .drive(options.initial_state())
        .await
}

/// Resume the run recorded in `out_dir`, from the stage it reached.
pub async fn resume(out_dir: &Path, quiet: bool) -> Result<RunReport> {
    let state = RunState::load(out_dir)?;
    let agent = build_agent(state.agent.kind, state.agent.command.as_deref())?;
    Engine::new(agent).quiet(quiet).drive(state).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{
        DestroyRequest, DestroyResponse, ForgeRequest, ForgeResponse, JudgeRequest, JudgeResponse,
        SpecRequest, SpecResponse,
    };
    use async_trait::async_trait;

    fn options(out: &Path) -> ForgeOptions {
        ForgeOptions {
            prompt: "turn text into slugs".to_string(),
            out_dir: out.to_path_buf(),
            stack: SUPPORTED_STACK.to_string(),
            max_rounds: DEFAULT_MAX_ROUNDS,
            agent: AgentKind::Template,
            agent_cmd: None,
            seal: false,
        }
    }

    #[test]
    fn refuses_a_stack_it_cannot_forge() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut options = options(&dir.path().join("crate"));
        options.stack = "python".to_string();
        let error = options.validate().expect_err("python must be refused");
        assert!(error.to_string().contains("unsupported stack `python`"));
        assert!(error.to_string().contains("no fallback"));
    }

    #[test]
    fn a_subprocess_run_needs_a_command() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut options = options(&dir.path().join("crate"));
        options.agent = AgentKind::Subprocess;
        let error = options
            .validate()
            .expect_err("missing command must be refused");
        assert!(error.to_string().contains("--agent-cmd"));
    }

    #[test]
    fn refuses_to_overwrite_a_recorded_run() {
        let dir = tempfile::tempdir().expect("temp dir");
        let state = crate::testsupport::state(dir.path());
        state.save().expect("save");
        let error = options(dir.path())
            .validate()
            .expect_err("existing run must be refused");
        assert!(error.to_string().contains("metafuge resume"));
    }

    #[test]
    fn refuses_a_non_empty_directory() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("keep.txt"), "user data").expect("write");
        let error = options(dir.path())
            .validate()
            .expect_err("non-empty directory must be refused");
        assert!(error.to_string().contains("not empty"));
    }

    /// A template agent that corrupts one source file, so the crucible fails
    /// every round. Used to prove the engine owns and enforces the round cap.
    struct BrokenForger {
        inner: TemplateAgent,
    }

    #[async_trait]
    impl MetafugeAgent for BrokenForger {
        fn describe(&self) -> AgentDescription {
            self.inner.describe()
        }

        async fn spec(&self, request: &SpecRequest) -> Result<SpecResponse> {
            self.inner.spec(request).await
        }

        async fn forge(&self, request: &ForgeRequest) -> Result<ForgeResponse> {
            let mut response = self.inner.forge(request).await?;
            for file in &mut response.files {
                if file.path == "src/slug.rs" {
                    file.contents =
                        "pub fn slugify(input: &str) -> String { let broken: u8 = \"no\"; input.to_string() }\n"
                            .to_string();
                }
            }
            Ok(response)
        }

        async fn destroy(&self, request: &DestroyRequest) -> Result<DestroyResponse> {
            self.inner.destroy(request).await
        }

        async fn judge(&self, request: &JudgeRequest) -> Result<JudgeResponse> {
            self.inner.judge(request).await
        }
    }

    #[tokio::test]
    async fn the_round_cap_fails_the_run_with_the_telemetry_dir_last() {
        let dir = tempfile::tempdir().expect("temp dir");
        let out = dir.path().join("crate");
        let mut options = options(&out);
        options.max_rounds = 1;
        options.validate().expect("valid options");

        let agent: Arc<dyn MetafugeAgent> = Arc::new(BrokenForger {
            inner: TemplateAgent,
        });
        let error = Engine::new(agent)
            .quiet(true)
            .drive(options.initial_state())
            .await
            .expect_err("a permanently broken crate must exhaust the cap");
        let message = format!("{error:#}");
        let telemetry = out.join(".metafuge").join("telemetry");
        assert_eq!(
            message.lines().last().expect("last line"),
            telemetry.display().to_string()
        );

        let state = RunState::load(&out).expect("recorded state");
        assert_eq!(state.status, RunStatus::Failed);
        assert!(
            state
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("round cap")
        );
        assert!(telemetry.join("crucible-1.json").is_file());
    }
}
