use std::collections::BTreeSet;
use std::sync::Arc;

use anyhow::{Context, Result};

use crate::agent::JudgeRequest;
use crate::director_rules;
use crate::manifest::Manifest;
use crate::run::{Run, Stage};
use crate::stages::StageOutcome;
use crate::support;
use crate::verdict::{JudgeDecision, JudgeSource, Verdict, VerdictStatus, Violation};

/// Director stage: the gate between a green crucible and bare metal.
///
/// Mechanical rules run first and are not negotiable. Only when they are clean
/// is any agent asked to judge, and that judgment is labelled as the agent's,
/// never as a mechanical rule.
pub async fn run(run: &mut Run) -> Result<StageOutcome> {
    let manifest = run.manifest()?.clone();
    let files =
        support::read_source_files(&run.state.out_dir, &manifest.architecture_blueprint.tree)?;
    let mechanical = director_rules::inspect(&files);
    let round = run.state.round;

    if !mechanical.is_empty() {
        let verdict = Verdict {
            status: VerdictStatus::RejectedByOverlord,
            round,
            judged_by: JudgeSource::Mechanical,
            rationale: format!(
                "mechanical gate rejected {} structural violation(s); these are deterministic text rules, not model judgment",
                mechanical.len()
            ),
            violations: mechanical,
            at: support::now(),
        };
        run.report(verdict.status.status_line());
        return reject(run, &manifest, verdict);
    }

    let agent = Arc::clone(&run.agent);
    let response = agent
        .judge(&JudgeRequest {
            round,
            manifest: manifest.clone(),
            source_files: files,
            mechanical_violations: Vec::new(),
        })
        .await
        .context("the director agent failed")?;

    let approved = response.decision == JudgeDecision::Approve && response.violations.is_empty();
    let verdict = Verdict {
        status: if approved {
            VerdictStatus::ApprovedForBareMetal
        } else {
            VerdictStatus::RejectedByOverlord
        },
        round,
        judged_by: JudgeSource::Agent,
        violations: response.violations,
        rationale: response.rationale,
        at: support::now(),
    };
    run.report(verdict.status.status_line());

    if verdict.status == VerdictStatus::ApprovedForBareMetal {
        run.state.verdict = Some(verdict);
        run.save()?;
        return Ok(StageOutcome::Advance(Stage::Seal));
    }
    reject(run, &manifest, verdict)
}

fn reject(run: &mut Run, manifest: &Manifest, verdict: Verdict) -> Result<StageOutcome> {
    let targets = repair_targets(&verdict.violations, manifest);
    let diagnostic = render_violations(&verdict.violations);
    run.state.verdict = Some(verdict);
    run.queue_repair(&targets, &diagnostic)?;
    run.save()?;
    run.report(format!(
        "director: repair queued for {}",
        targets.join(", ")
    ));
    Ok(StageOutcome::Retry)
}

/// Route each violation to the task that owns the file. A violation no task
/// owns re-opens every forger, which the round cap bounds.
fn repair_targets(violations: &[Violation], manifest: &Manifest) -> Vec<String> {
    let mut ids = BTreeSet::new();
    for violation in violations {
        if let Some((id, _)) = manifest.owner_of(&violation.file) {
            ids.insert(id.to_string());
        }
    }
    if ids.is_empty() {
        for task in &manifest.forger_tasks {
            ids.insert(task.id.clone());
        }
    }
    ids.into_iter().collect()
}

fn render_violations(violations: &[Violation]) -> String {
    if violations.is_empty() {
        return "director rejected the build without file:line violations".to_string();
    }
    violations
        .iter()
        .take(40)
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::TaskStatus;
    use crate::verdict::JudgeSource;

    #[tokio::test]
    async fn approves_a_clean_tree() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut run = crate::testsupport::run(dir.path());
        crate::testsupport::advance_through(&mut run, Stage::Director).await;

        let outcome = super::run(&mut run).await.expect("director stage");
        assert_eq!(outcome, StageOutcome::Advance(Stage::Seal));
        let verdict = run.state.verdict.as_ref().expect("verdict recorded");
        assert!(verdict.approved());
        assert_eq!(
            verdict.status.status_line(),
            "STATUS: APPROVED_FOR_BARE_METAL"
        );
        assert_eq!(verdict.judged_by, JudgeSource::Agent);
        assert!(verdict.rationale.contains("no structural model judgment"));
    }

    #[tokio::test]
    async fn mechanical_violations_reject_and_reopen_the_owning_task() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut run = crate::testsupport::run(dir.path());
        crate::testsupport::advance_through(&mut run, Stage::Director).await;

        let source = std::fs::read_to_string(dir.path().join("src/slug.rs")).expect("source");
        std::fs::write(
            dir.path().join("src/slug.rs"),
            format!(
                "{source}\n/// a shipped path with lazy error handling\npub fn leak() -> u8 {{\n    let value: Option<u8> = None;\n    value.unwrap()\n}}\n"
            ),
        )
        .expect("inject a violation");

        let outcome = super::run(&mut run).await.expect("director stage");
        assert_eq!(outcome, StageOutcome::Retry);
        let verdict = run.state.verdict.as_ref().expect("verdict recorded");
        assert_eq!(verdict.status.status_line(), "STATUS: REJECTED_BY_OVERLORD");
        assert_eq!(verdict.judged_by, JudgeSource::Mechanical);
        assert!(verdict.rationale.contains("not model judgment"));
        let violation = verdict
            .violations
            .iter()
            .find(|violation| violation.rule == "panic-outside-tests")
            .expect("unwrap must be reported");
        assert_eq!(violation.file, "src/slug.rs");
        assert!(violation.line > 0);

        assert_eq!(run.task_status("f2"), Some(TaskStatus::Pending));
        assert_eq!(run.task_status("f1"), Some(TaskStatus::Complete));
        let diagnostic = run
            .state
            .task("f2")
            .and_then(|task| task.diagnostic.as_deref())
            .expect("diagnostic");
        assert!(diagnostic.contains("src/slug.rs:"));
        assert!(diagnostic.contains("unwrap"));
    }
}
