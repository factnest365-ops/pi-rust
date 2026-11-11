use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use chrono::SecondsFormat;
use serde::{Deserialize, Serialize};

use crate::director_rules::SourceFile;

/// One subprocess invocation, captured whole for telemetry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub command: String,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub timed_out: bool,
}

impl CommandResult {
    pub fn succeeded(&self) -> bool {
        !self.timed_out && self.exit_code == Some(0)
    }
}

/// Run a subprocess with a hard timeout.
///
/// A timeout is reported as a failed command rather than an error: the forge
/// records it and moves on instead of silently hanging.
pub async fn run_command(
    program: &str,
    args: &[&str],
    cwd: &Path,
    timeout: Duration,
) -> Result<CommandResult> {
    let display = if args.is_empty() {
        program.to_string()
    } else {
        format!("{program} {}", args.join(" "))
    };
    let mut command = tokio::process::Command::new(program);
    command
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("CARGO_TERM_COLOR", "never")
        .env_remove("CARGO_TARGET_DIR")
        .kill_on_drop(true);

    let started = Instant::now();
    let child = command
        .spawn()
        .with_context(|| format!("failed to spawn `{display}`"))?;
    let elapsed = |start: Instant| start.elapsed().as_millis() as u64;

    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(output) => {
            let output = output.with_context(|| format!("`{display}` could not be collected"))?;
            Ok(CommandResult {
                command: display,
                exit_code: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                duration_ms: elapsed(started),
                timed_out: false,
            })
        }
        Err(_) => Ok(CommandResult {
            command: display,
            exit_code: None,
            stdout: String::new(),
            stderr: format!("timed out after {}s", timeout.as_secs()),
            duration_ms: elapsed(started),
            timed_out: true,
        }),
    }
}

/// Reject any path that could escape the forge target directory.
pub fn safe_relative(path: &str) -> Result<PathBuf> {
    if path.trim().is_empty() {
        bail!("empty path");
    }
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        bail!("absolute path `{path}` is not allowed");
    }
    for component in candidate.components() {
        match component {
            Component::Normal(_) => {}
            Component::CurDir => bail!("path `{path}` is not normalized"),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                bail!("path `{path}` escapes the target directory")
            }
        }
    }
    Ok(candidate.to_path_buf())
}

/// Write a file atomically so an interrupted run never leaves a half-written
/// state record behind.
pub fn write_atomic(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("could not create `{}`", parent.display()))?;
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("record");
    let temporary = path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));
    std::fs::write(&temporary, contents)
        .with_context(|| format!("could not write `{}`", temporary.display()))?;
    std::fs::rename(&temporary, path)
        .with_context(|| format!("could not commit `{}`", path.display()))?;
    Ok(())
}

/// Write a worker-produced file, refusing any path the blueprint did not claim.
pub fn write_generated(base: &Path, tree: &[String], path: &str, contents: &str) -> Result<()> {
    let relative = safe_relative(path)?;
    if !tree.iter().any(|candidate| candidate == path) {
        bail!("worker attempted to write `{path}`, which is not in the architecture blueprint");
    }
    write_atomic(&base.join(relative), contents)
}

/// Read every Rust source claimed by the blueprint, for the director gate.
pub fn read_source_files(base: &Path, tree: &[String]) -> Result<Vec<SourceFile>> {
    let mut files = Vec::new();
    for path in tree {
        if !path.ends_with(".rs") {
            continue;
        }
        let relative = safe_relative(path)?;
        let absolute = base.join(relative);
        if !absolute.exists() {
            continue;
        }
        let contents = std::fs::read_to_string(&absolute)
            .with_context(|| format!("could not read `{}`", absolute.display()))?;
        files.push(SourceFile {
            path: path.clone(),
            contents,
        });
    }
    Ok(files)
}

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// Shorten a log for an error message without losing the head of the failure.
pub fn preview(text: &str, limit: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= limit {
        return trimmed.to_string();
    }
    let head: String = trimmed.chars().take(limit).collect();
    format!("{head}...")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_relative_accepts_nested_paths() {
        assert_eq!(
            safe_relative("src/nested/mod.rs").expect("valid"),
            PathBuf::from("src/nested/mod.rs")
        );
    }

    #[test]
    fn safe_relative_rejects_escapes() {
        assert!(safe_relative("../escape.rs").is_err());
        assert!(safe_relative("/etc/passwd").is_err());
        assert!(safe_relative("./relative.rs").is_err());
        assert!(safe_relative("").is_err());
    }

    #[test]
    fn atomic_write_replaces_existing_content() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("nested/record.json");
        write_atomic(&path, "first").expect("write");
        write_atomic(&path, "second").expect("rewrite");
        assert_eq!(std::fs::read_to_string(&path).expect("read"), "second");
    }

    #[test]
    fn generated_files_must_be_in_the_blueprint() {
        let dir = tempfile::tempdir().expect("temp dir");
        let tree = vec!["src/lib.rs".to_string()];
        assert!(write_generated(dir.path(), &tree, "src/lib.rs", "// ok").is_ok());
        let error = write_generated(dir.path(), &tree, "src/rogue.rs", "// no")
            .expect_err("rogue file must be rejected");
        assert!(error.to_string().contains("src/rogue.rs"));
    }

    #[test]
    fn preview_truncates_without_splitting_characters() {
        assert_eq!(preview("abcdef", 10), "abcdef");
        assert_eq!(preview("abcdef", 3), "abc...");
        assert_eq!(preview("éééé", 2), "éé...");
    }
}
