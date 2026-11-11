use std::time::Duration;

use anyhow::{Context, Result, bail};

use crate::run::Run;
use crate::stages::StageOutcome;
use crate::support;
use crate::verdict::VerdictStatus;

const GIT_TIMEOUT: Duration = Duration::from_secs(120);

/// Seal stage: the only stage that writes outside the forge target's files.
///
/// It runs only when `--seal` was requested and the director approved. It
/// initializes a local repository if needed, stages everything, and makes one
/// commit. It never pushes, tags, or touches a remote.
pub async fn run(run: &mut Run) -> Result<StageOutcome> {
    if let Some(commit) = run.state.sealed_commit.clone() {
        run.report(format!("seal: already committed at {commit}"));
        return Ok(StageOutcome::Complete);
    }
    if !run.state.seal_requested {
        run.report("seal: not requested; worktree left uncommitted");
        return Ok(StageOutcome::Complete);
    }
    let verdict = run
        .state
        .verdict
        .as_ref()
        .context("seal refuses to run without a director verdict")?;
    if verdict.status != VerdictStatus::ApprovedForBareMetal {
        return Ok(StageOutcome::Fail(
            "seal refused: the director did not approve this build".to_string(),
        ));
    }
    let manifest = run.manifest()?.clone();
    let out = run.state.out_dir.clone();

    if !out.join(".git").exists() {
        let result = support::run_command("git", &["init", "--quiet"], &out, GIT_TIMEOUT).await?;
        ensure(&result, "git init")?;
        run.report("seal: initialized a local git repository");
    }

    let add = support::run_command("git", &["add", "-A"], &out, GIT_TIMEOUT).await?;
    ensure(&add, "git add")?;

    // Deterministic identity for forged commits: a CI box without a git identity
    // must still be able to seal, and the forge commit is not a personal commit.
    let message = format!(
        "forge: {} (round {}/{})",
        manifest.title, run.state.round, run.state.max_rounds
    );
    let commit = support::run_command(
        "git",
        &[
            "-c",
            "user.name=METAFUGE",
            "-c",
            "user.email=metafuge@localhost",
            "commit",
            "--quiet",
            "-m",
            &message,
        ],
        &out,
        GIT_TIMEOUT,
    )
    .await?;
    ensure(&commit, "git commit")?;

    let head = support::run_command("git", &["rev-parse", "HEAD"], &out, GIT_TIMEOUT).await?;
    ensure(&head, "git rev-parse")?;
    let hash = head.stdout.trim().to_string();
    if hash.is_empty() {
        bail!("git rev-parse returned no commit hash after sealing");
    }
    run.state.sealed_commit = Some(hash.clone());
    run.save()?;
    run.report(format!(
        "seal: local commit {hash}; nothing pushed or tagged"
    ));
    Ok(StageOutcome::Complete)
}

fn ensure(result: &support::CommandResult, label: &str) -> Result<()> {
    if result.succeeded() {
        return Ok(());
    }
    bail!("{label} failed: {}", support::preview(&result.stderr, 400))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::Stage;
    use crate::support;
    use crate::verdict::{JudgeSource, Verdict, Violation};

    #[tokio::test]
    async fn seals_only_when_requested_and_only_after_approval() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut run = crate::testsupport::run(dir.path());
        crate::testsupport::advance_through(&mut run, Stage::Seal).await;

        let outcome = super::run(&mut run).await.expect("unsealed run");
        assert_eq!(outcome, StageOutcome::Complete);
        assert!(!dir.path().join(".git").exists());
        assert!(run.state.sealed_commit.is_none());

        run.state.seal_requested = true;
        let outcome = super::run(&mut run).await.expect("seal");
        assert_eq!(outcome, StageOutcome::Complete);
        assert!(dir.path().join(".git").is_dir());
        let commit = run.state.sealed_commit.clone().expect("commit recorded");
        assert_eq!(commit.len(), 40);

        let log = support::run_command(
            "git",
            &["log", "-1", "--pretty=%s"],
            dir.path(),
            Duration::from_secs(30),
        )
        .await
        .expect("git log");
        assert!(log.succeeded());
        assert!(log.stdout.contains("slugify"));
        assert!(log.stdout.contains("round 1/3"));
    }

    #[tokio::test]
    async fn refuses_to_seal_a_rejected_build() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut run = crate::testsupport::run(dir.path());
        run.state.seal_requested = true;
        run.state.verdict = Some(Verdict {
            status: VerdictStatus::RejectedByOverlord,
            round: 1,
            judged_by: JudgeSource::Mechanical,
            violations: vec![Violation::new(
                "src/lib.rs",
                1,
                "panic-outside-tests",
                "unwrap outside tests",
            )],
            rationale: "mechanical gate rejected the build".to_string(),
            at: support::now(),
        });

        let outcome = super::run(&mut run).await.expect("seal stage");
        assert!(matches!(outcome, StageOutcome::Fail(_)));
        assert!(!dir.path().join(".git").exists());
        assert!(run.state.sealed_commit.is_none());
    }
}
