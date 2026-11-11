use std::path::Path;
use std::sync::Arc;

use crate::agent::{AgentKind, TemplateAgent};
use crate::manifest::Manifest;
use crate::run::{AgentState, Run, RunState, RunStatus, Stage};
use crate::support;

pub(crate) fn manifest() -> Manifest {
    crate::agent::template::fixture_manifest()
}

pub(crate) fn state(out: &Path) -> RunState {
    RunState {
        schema: crate::run::RUN_SCHEMA,
        run_id: "test-run".to_string(),
        prompt: "slugify arbitrary text".to_string(),
        stack: "rust".to_string(),
        out_dir: out.to_path_buf(),
        stage: Stage::Spec,
        status: RunStatus::Running,
        round: 1,
        max_rounds: 3,
        seal_requested: false,
        agent: AgentState {
            kind: AgentKind::Template,
            command: None,
        },
        tasks: Vec::new(),
        manifest: None,
        verdict: None,
        telemetry: Vec::new(),
        sealed_commit: None,
        error: None,
        created_at: support::now(),
        updated_at: support::now(),
    }
}

pub(crate) fn run(out: &Path) -> Run {
    Run::new(state(out), Arc::new(TemplateAgent))
}

/// Drive a run through the named stages in order, stopping before the next one.
pub(crate) async fn advance_through(run: &mut Run, last: Stage) {
    loop {
        let stage = run.state.stage;
        if stage == last {
            return;
        }
        let outcome = crate::stages::dispatch(stage, run)
            .await
            .unwrap_or_else(|error| panic!("stage {} failed: {error:#}", stage.as_str()));
        match outcome {
            crate::stages::StageOutcome::Advance(next) => {
                run.state.stage = next;
                run.save().expect("save state");
            }
            other => panic!("unexpected outcome at {}: {other:?}", stage.as_str()),
        }
    }
}
