use std::sync::Arc;

use anyhow::{Context, Result};

use crate::agent::ForgeRequest;
use crate::run::{Run, Stage, TaskStatus};
use crate::stages::StageOutcome;
use crate::support;

/// Forge stage: issue only the tasks that are pending, each with its own
/// contract refs and, on a repair round, only the trimmed diagnostic.
pub async fn run(run: &mut Run) -> Result<StageOutcome> {
    let manifest = run.manifest()?.clone();
    let out = run.state.out_dir.clone();
    let tree = manifest.architecture_blueprint.tree.clone();
    let agent = Arc::clone(&run.agent);
    let mut issued = 0usize;

    for task in &manifest.forger_tasks {
        if run.task_status(&task.id) == Some(TaskStatus::Complete) {
            continue;
        }
        let diagnostic = run.take_diagnostic(&task.id);
        run.set_task_status(&task.id, TaskStatus::InFlight)?;
        run.save()?;

        let request = ForgeRequest {
            round: run.state.round,
            task: task.clone(),
            contracts: manifest.shared_contracts.clone(),
            blueprint: manifest.architecture_blueprint.clone(),
            diagnostic,
        };
        let response = agent
            .forge(&request)
            .await
            .with_context(|| format!("forger `{}` failed", task.id))?;
        for file in &response.files {
            support::write_generated(&out, &tree, &file.path, &file.contents).with_context(
                || format!("forger `{}` returned a file outside the blueprint", task.id),
            )?;
        }
        run.set_task_status(&task.id, TaskStatus::Complete)?;
        run.save()?;
        issued += 1;
    }

    run.report(format!("forge: {issued} task(s) issued this round"));
    Ok(StageOutcome::Advance(Stage::Destroy))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn writes_every_forger_file_and_marks_tasks_complete() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut run = crate::testsupport::run(dir.path());
        crate::testsupport::advance_through(&mut run, Stage::Forge).await;

        let outcome = super::run(&mut run).await.expect("forge stage");
        assert_eq!(outcome, StageOutcome::Advance(Stage::Destroy));
        assert_eq!(run.task_status("f1"), Some(TaskStatus::Complete));
        assert_eq!(run.task_status("f2"), Some(TaskStatus::Complete));

        let root = std::fs::read_to_string(dir.path().join("src/lib.rs")).expect("lib");
        assert!(root.contains("pub use slug::slugify;"));
        let slug = std::fs::read_to_string(dir.path().join("src/slug.rs")).expect("slug");
        assert!(slug.contains("pub fn slugify"));
        assert!(!slug.contains(".unwrap()"));
    }

    #[tokio::test]
    async fn a_second_pass_reissues_nothing() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut run = crate::testsupport::run(dir.path());
        crate::testsupport::advance_through(&mut run, Stage::Forge).await;
        super::run(&mut run).await.expect("first forge");
        let attempts: Vec<u32> = run.state.tasks.iter().map(|task| task.attempts).collect();
        super::run(&mut run).await.expect("second forge");
        let after: Vec<u32> = run.state.tasks.iter().map(|task| task.attempts).collect();
        assert_eq!(attempts, after, "complete tasks must not be re-issued");
    }
}
