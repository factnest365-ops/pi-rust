use anyhow::{anyhow, Result};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct StateSynchronizer {
    pub tau_dir: PathBuf,
    pub git_enabled: bool,
}

impl StateSynchronizer {
    pub fn new(tau_dir: PathBuf) -> Self {
        Self {
            tau_dir,
            git_enabled: true,
        }
    }

    /// Default system-wide Tau state directory (~/.tau)
    pub fn default_dir() -> PathBuf {
        dirs::home_dir()
            .map(|h| h.join(".tau"))
            .unwrap_or_else(|| PathBuf::from(".tau"))
    }

    /// Initializes Git repository in the target Tau directory if not already present.
    pub fn init_repo_if_needed(&self) -> Result<()> {
        if !self.tau_dir.exists() {
            fs::create_dir_all(&self.tau_dir)?;
        }

        let git_dir = self.tau_dir.join(".git");
        if !git_dir.exists() {
            let output = Command::new("git")
                .args(["init", "-b", "main"])
                .current_dir(&self.tau_dir)
                .output()?;

            if !output.status.success() {
                // Fallback for older git versions that do not support -b
                let _ = Command::new("git")
                    .arg("init")
                    .current_dir(&self.tau_dir)
                    .output()?;
            }

            // Write initial gitignore
            let gitignore_path = self.tau_dir.join(".gitignore");
            if !gitignore_path.exists() {
                fs::write(
                    &gitignore_path,
                    "*.sock\n*.tmp\nworktrees/\n*.lock\n",
                )?;
            }

            // Configure local git identity if needed
            let _ = Command::new("git")
                .args(["config", "user.name", "Tau Autonomous Harness"])
                .current_dir(&self.tau_dir)
                .output();

            let _ = Command::new("git")
                .args(["config", "user.email", "tau@antigravity.ai"])
                .current_dir(&self.tau_dir)
                .output();

            // Initial commit
            let _ = Command::new("git")
                .args(["add", "."])
                .current_dir(&self.tau_dir)
                .output();

            let _ = Command::new("git")
                .args(["commit", "-m", "chore: initialize Tau cognitive mind repository"])
                .current_dir(&self.tau_dir)
                .output();
        }

        Ok(())
    }

    /// Automatically commits state changes (crystallized skills, vault memories, reflexion rules).
    pub fn commit_state_change(&self, subject: &str, details: Option<&str>) -> Result<String> {
        if !self.git_enabled {
            return Ok("Git sync is disabled".to_string());
        }

        self.init_repo_if_needed()?;

        // git add .
        let add_out = Command::new("git")
            .args(["add", "."])
            .current_dir(&self.tau_dir)
            .output()?;

        if !add_out.status.success() {
            return Err(anyhow!(
                "Git add failed: {}",
                String::from_utf8_lossy(&add_out.stderr)
            ));
        }

        let message = if let Some(d) = details {
            format!("{}\n\n{}", subject, d)
        } else {
            subject.to_string()
        };

        let commit_out = Command::new("git")
            .args(["commit", "-m", &message])
            .current_dir(&self.tau_dir)
            .output()?;

        if !commit_out.status.success() {
            let stderr = String::from_utf8_lossy(&commit_out.stderr);
            let stdout = String::from_utf8_lossy(&commit_out.stdout);
            if stdout.contains("nothing to commit") || stderr.contains("nothing to commit") {
                return Ok("No changes to commit".to_string());
            }
            return Err(anyhow!("Git commit failed: {} {}", stdout, stderr));
        }

        Ok(format!("State committed: {}", subject))
    }

    /// Returns git log summary of the mind repository.
    pub fn log_summary(&self, count: usize) -> Result<String> {
        let output = Command::new("git")
            .args(["log", &format!("-n{}", count), "--oneline"])
            .current_dir(&self.tau_dir)
            .output()?;

        if !output.status.success() {
            return Err(anyhow!(
                "Git log failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_state_synchronizer_init_and_commit() {
        let tmp = tempdir().unwrap();
        let sync = StateSynchronizer::new(tmp.path().to_path_buf());

        // 1. Init
        sync.init_repo_if_needed().unwrap();
        assert!(tmp.path().join(".git").exists());
        assert!(tmp.path().join(".gitignore").exists());

        // 2. Add a skill file
        let skill_dir = tmp.path().join("skills/rust-perf");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "# Rust Performance\nUse zero-copy SIMD.\n").unwrap();

        // 3. Commit mutation
        let res = sync.commit_state_change("feat(skill): crystallize rust-perf", None).unwrap();
        assert!(res.contains("State committed"));

        // 4. Check log
        let log = sync.log_summary(5).unwrap();
        assert!(log.contains("rust-perf"));
    }
}
