use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use crate::deps;
use crate::diagnostics;
use crate::run::{Run, Stage};
use crate::stages::StageOutcome;
use crate::support;
use crate::telemetry::TelemetryRecord;

/// Hard budget for one cargo gate. A generated crate has no dependencies by
/// default, so this only fires on a pathological target.
pub const CRUCIBLE_TIMEOUT: Duration = Duration::from_secs(900);

#[derive(Debug, Clone, Copy)]
struct Gate {
    label: &'static str,
    args: &'static [&'static str],
}

/// Build first, then test, then lint. The first failure stops the sequence:
/// running tests against a crate that does not compile only manufactures noise.
const GATES: [Gate; 3] = [
    Gate {
        label: "build",
        args: &["build", "--quiet"],
    },
    Gate {
        label: "test",
        args: &["test", "--quiet"],
    },
    Gate {
        label: "clippy",
        args: &["clippy", "--all-targets", "--", "-D", "warnings"],
    },
];

/// Crucible stage: prove the generated crate compiles, passes its adversarial
/// tests, and is lint-clean. Every gate is recorded whole in telemetry.
pub async fn run(run: &mut Run) -> Result<StageOutcome> {
    let manifest = run.manifest()?.clone();
    let out = run.state.out_dir.clone();
    let round = run.state.round;
    let telemetry_dir = run.telemetry_dir();
    std::fs::create_dir_all(&telemetry_dir)
        .with_context(|| format!("could not create `{}`", telemetry_dir.display()))?;

    let started = Instant::now();
    let mut record = TelemetryRecord::new(
        &run.state.run_id,
        Stage::Crucible.as_str(),
        round,
        support::now(),
    );

    let violations = deps::check_file(&out.join("Cargo.toml"), &manifest.allowed_deps)?;
    if !violations.is_empty() {
        let detail = violations
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ");
        record.notes.push(format!("dependency allowlist: {detail}"));
        record.duration_ms = started.elapsed().as_millis() as u64;
        let path = record.write(&telemetry_dir)?;
        run.record_telemetry(path);
        run.save()?;
        return Ok(StageOutcome::Fail(format!(
            "dependency allowlist violated: {detail}"
        )));
    }

    let mut failure: Option<(&'static str, String)> = None;
    for gate in GATES {
        let result = support::run_command("cargo", gate.args, &out, CRUCIBLE_TIMEOUT)
            .await
            .with_context(|| format!("could not run the {} gate", gate.label))?;
        let passed = result.succeeded();
        record.notes.push(format!(
            "{}: {} in {} ms",
            gate.label,
            if passed { "pass" } else { "fail" },
            result.duration_ms
        ));
        if !passed {
            let mut log = result.stdout.clone();
            if !result.stderr.trim().is_empty() {
                if !log.is_empty() {
                    log.push('\n');
                }
                log.push_str(&result.stderr);
            }
            failure = Some((gate.label, log));
            record.commands.push(result);
            break;
        }
        record.commands.push(result);
    }
    record.duration_ms = started.elapsed().as_millis() as u64;
    record.ok = failure.is_none();

    if let Some((label, log)) = failure {
        let diagnostic = diagnostics::trim(&log);
        let targets = diagnostics::map_failures(&log, &manifest);
        let ids = targets.ids();
        record
            .notes
            .push(format!("{label} failure mapped to: {}", ids.join(", ")));
        let path = record.write(&telemetry_dir)?;
        run.record_telemetry(path);
        if !ids.is_empty() {
            run.queue_repair(&ids, &diagnostic)?;
        }
        run.save()?;
        run.report(format!(
            "crucible: {label} failed in round {round}; repair queued for {}",
            ids.join(", ")
        ));
        return Ok(StageOutcome::Retry);
    }

    let path = record.write(&telemetry_dir)?;
    run.record_telemetry(path);
    run.save()?;
    run.report("crucible: build, test, clippy all clean");
    Ok(StageOutcome::Advance(Stage::Director))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::TaskStatus;

    #[tokio::test]
    async fn a_clean_crate_advances_and_records_telemetry() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut run = crate::testsupport::run(dir.path());
        crate::testsupport::advance_through(&mut run, Stage::Crucible).await;

        let outcome = super::run(&mut run).await.expect("crucible stage");
        assert_eq!(outcome, StageOutcome::Advance(Stage::Director));

        let path = dir
            .path()
            .join(".metafuge")
            .join("telemetry")
            .join("crucible-1.json");
        assert!(path.is_file(), "telemetry must be written");
        let record: TelemetryRecord =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("telemetry"))
                .expect("parse telemetry");
        assert!(record.ok);
        assert_eq!(record.commands.len(), GATES.len());
        assert!(record.commands.iter().all(|command| command.succeeded()));
        assert!(
            record
                .commands
                .iter()
                .all(|command| command.duration_ms < CRUCIBLE_TIMEOUT.as_millis() as u64)
        );
        assert!(run.state.telemetry.contains(&path));
    }

    #[tokio::test]
    async fn a_compile_error_queues_the_owning_forger_and_retries() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut run = crate::testsupport::run(dir.path());
        crate::testsupport::advance_through(&mut run, Stage::Crucible).await;

        std::fs::write(
            dir.path().join("src/slug.rs"),
            "pub fn slugify(input: &str) -> String {\n    let value: u8 = \"broken\";\n    input.to_string()\n}\n",
        )
        .expect("corrupt the source");

        let outcome = super::run(&mut run).await.expect("crucible stage");
        assert_eq!(outcome, StageOutcome::Retry);
        assert_eq!(run.task_status("f2"), Some(TaskStatus::Pending));
        assert_eq!(run.task_status("f1"), Some(TaskStatus::Complete));

        let diagnostic = run
            .state
            .task("f2")
            .and_then(|task| task.diagnostic.as_deref())
            .expect("the owning task carries the diagnostic");
        assert!(diagnostic.contains("error"));
        assert!(diagnostic.lines().count() <= diagnostics::MAX_DIAGNOSTIC_LINES);
    }
}
