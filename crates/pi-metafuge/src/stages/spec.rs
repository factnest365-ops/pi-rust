use std::sync::Arc;

use anyhow::{Context, Result};

use crate::agent::SpecRequest;
use crate::manifest::MANIFEST_SCHEMA;
use crate::run::{Run, Stage};
use crate::stages::StageOutcome;
use crate::support;

/// Chief-of-staff stage: turn the raw prompt into a blueprint, a contract set,
/// and isolated payloads for both sides.
pub async fn run(run: &mut Run) -> Result<StageOutcome> {
    run.report("chief-of-staff: shaping the prompt into architecture");
    let agent = Arc::clone(&run.agent);
    let request = SpecRequest {
        schema: MANIFEST_SCHEMA,
        prompt: run.state.prompt.clone(),
        stack: run.state.stack.clone(),
    };
    let response = agent.spec(&request).await?;
    let manifest = response.manifest;
    manifest
        .validate()
        .with_context(|| "chief-of-staff manifest rejected by the contract gate")?;

    let summary = format!(
        "blueprint accepted: {} paths, {} contracts, {} forger tasks, {} destroyer tasks, {} allowed deps",
        manifest.architecture_blueprint.tree.len(),
        manifest.shared_contracts.len(),
        manifest.forger_tasks.len(),
        manifest.destroyer_tasks.len(),
        manifest.allowed_deps.len()
    );
    run.state.manifest = Some(manifest.clone());
    run.ensure_tasks(&manifest);
    support::write_atomic(
        &run.metafuge_dir().join("manifest.json"),
        &serde_json::to_string_pretty(&manifest)?,
    )?;
    run.save()?;
    run.report(summary);
    Ok(StageOutcome::Advance(Stage::Scaffold))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::TaskStatus;

    #[tokio::test]
    async fn produces_a_valid_manifest_and_registers_tasks() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut run = crate::testsupport::run(dir.path());
        let outcome = super::run(&mut run).await.expect("spec stage");
        assert_eq!(outcome, StageOutcome::Advance(Stage::Scaffold));
        let manifest = run.manifest().expect("manifest").clone();
        assert_eq!(manifest.schema, MANIFEST_SCHEMA);
        assert_eq!(run.state.tasks.len(), manifest.total_tasks());
        assert!(
            run.state
                .tasks
                .iter()
                .all(|task| task.status == TaskStatus::Pending)
        );
        assert!(run.metafuge_dir().join("manifest.json").exists());
        assert_eq!(run.state.stage, Stage::Spec);
    }
}
