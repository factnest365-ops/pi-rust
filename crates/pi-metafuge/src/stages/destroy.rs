use std::sync::Arc;

use anyhow::{Context, Result};

use crate::agent::DestroyRequest;
use crate::run::{Run, Stage, TaskStatus};
use crate::stages::StageOutcome;
use crate::support;

/// Destroy stage: issue only the destroyer tasks that are pending, each with the
/// shared contracts and never with forger source.
pub async fn run(run: &mut Run) -> Result<StageOutcome> {
    let manifest = run.manifest()?.clone();
    let out = run.state.out_dir.clone();
    let tree = manifest.architecture_blueprint.tree.clone();
    let agent = Arc::clone(&run.agent);
    let mut issued = 0usize;

    for task in &manifest.destroyer_tasks {
        if run.task_status(&task.id) == Some(TaskStatus::Complete) {
            continue;
        }
        let diagnostic = run.take_diagnostic(&task.id);
        run.set_task_status(&task.id, TaskStatus::InFlight)?;
        run.save()?;

        let request = DestroyRequest {
            round: run.state.round,
            task: task.clone(),
            contracts: manifest.shared_contracts.clone(),
            blueprint: manifest.architecture_blueprint.clone(),
            diagnostic,
        };
        let response = agent
            .destroy(&request)
            .await
            .with_context(|| format!("destroyer `{}` failed", task.id))?;
        for file in &response.files {
            support::write_generated(&out, &tree, &file.path, &file.contents).with_context(
                || {
                    format!(
                        "destroyer `{}` returned a file outside the blueprint",
                        task.id
                    )
                },
            )?;
        }
        run.set_task_status(&task.id, TaskStatus::Complete)?;
        run.save()?;
        issued += 1;
    }

    run.report(format!(
        "destroy: {issued} adversarial test task(s) issued this round"
    ));
    Ok(StageOutcome::Advance(Stage::Crucible))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn writes_adversarial_tests_and_marks_destroyers_complete() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut run = crate::testsupport::run(dir.path());
        crate::testsupport::advance_through(&mut run, Stage::Destroy).await;

        let outcome = super::run(&mut run).await.expect("destroy stage");
        assert_eq!(outcome, StageOutcome::Advance(Stage::Crucible));
        for id in ["d1", "d2", "d3"] {
            assert_eq!(run.task_status(id), Some(TaskStatus::Complete));
        }

        let integration =
            std::fs::read_to_string(dir.path().join("tests/integration.rs")).expect("test file");
        assert!(integration.contains("use slugify::slugify;"));
        assert!(
            !integration.contains("pub fn slugify"),
            "a destroyer must not see forger source"
        );
        assert!(dir.path().join("tests/boundary.rs").is_file());
        assert!(dir.path().join("tests/fuzz.rs").is_file());
    }

    #[tokio::test]
    async fn a_second_pass_reissues_nothing() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut run = crate::testsupport::run(dir.path());
        crate::testsupport::advance_through(&mut run, Stage::Destroy).await;
        super::run(&mut run).await.expect("first destroy");
        let attempts: Vec<u32> = run.state.tasks.iter().map(|task| task.attempts).collect();
        super::run(&mut run).await.expect("second destroy");
        let after: Vec<u32> = run.state.tasks.iter().map(|task| task.attempts).collect();
        assert_eq!(attempts, after, "complete tasks must not be re-issued");
    }
}
