use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorktreeInfo {
    pub path: String,
    pub head: String,
    pub branch: Option<String>,
    pub bare: bool,
    pub locked: bool,
    pub prunable: bool,
}

pub fn git_cmd() -> Command {
    let mut cmd = Command::new("git");
    cmd.env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_PAGER", "cat");
    cmd
}

pub struct GitTool;

impl GitTool {
    pub fn execute(args: &Value) -> Result<String> {
        let action = args["action"]
            .as_str()
            .or_else(|| args["command"].as_str())
            .unwrap_or("status");

        match action {
            "status" => Self::git_status(),
            "diff" => {
                let staged = args["staged"].as_bool().unwrap_or(false);
                let file = args["file"].as_str().or_else(|| args["path"].as_str());
                Self::git_diff(staged, file)
            }
            "log" => {
                let count = args["count"].as_u64().unwrap_or(10) as usize;
                Self::git_log(count)
            }
            "branch" => Self::git_branch(),
            "commit_proposal" => Self::synthesize_commit_message(),
            "commit" => {
                let message = args["message"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing 'message' argument for git commit"))?;
                Self::git_commit(message)
            }
            "worktree_add" | "worktree_create" => {
                let task_id = args["task_id"]
                    .as_str()
                    .or_else(|| args["task"].as_str())
                    .or_else(|| args["id"].as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'task_id' for worktree creation"))?;
                let base_branch = args["base_branch"]
                    .as_str()
                    .or_else(|| args["branch"].as_str())
                    .unwrap_or("HEAD");
                let path = git_worktree_create(base_branch, task_id)?;
                Ok(format!(
                    "Created worktree for task '{}' at '{}' on branch 'pi-task-{}'",
                    task_id,
                    path.display(),
                    task_id
                ))
            }
            "worktree_remove" | "worktree_delete" => {
                let task_id = args["task_id"]
                    .as_str()
                    .or_else(|| args["task"].as_str())
                    .or_else(|| args["id"].as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'task_id' for worktree removal"))?;
                let force = args["force"].as_bool().unwrap_or(false);
                git_worktree_remove(task_id, force)?;
                Ok(format!("Successfully removed worktree for task '{}'", task_id))
            }
            "worktree_list" => {
                let worktrees = git_worktree_list()?;
                if worktrees.is_empty() {
                    Ok("No active worktrees found.".to_string())
                } else {
                    let mut out = format!("Active Git Worktrees ({}):\n", worktrees.len());
                    for wt in worktrees {
                        let branch_desc = wt.branch.as_deref().unwrap_or("(detached)");
                        let flags = match (wt.locked, wt.prunable, wt.bare) {
                            (true, _, _) => " [locked]",
                            (_, true, _) => " [prunable]",
                            (_, _, true) => " [bare]",
                            _ => "",
                        };
                        let short_head = if wt.head.len() >= 7 {
                            &wt.head[..7]
                        } else {
                            &wt.head
                        };
                        out.push_str(&format!(
                            "- {} | branch: {} | HEAD: {}{}\n",
                            wt.path, branch_desc, short_head, flags
                        ));
                    }
                    Ok(out.trim_end().to_string())
                }
            }
            "worktree_merge" => {
                let task_id = args["task_id"]
                    .as_str()
                    .or_else(|| args["task"].as_str())
                    .or_else(|| args["id"].as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'task_id' for worktree merge"))?;
                let target_branch = args["target_branch"]
                    .as_str()
                    .or_else(|| args["target"].as_str())
                    .unwrap_or("main");
                git_worktree_merge(task_id, target_branch)
            }
            _ => Err(anyhow::anyhow!(
                "Unknown git action '{}'. Supported actions: status, diff, log, branch, commit_proposal, commit, worktree_add, worktree_remove, worktree_list, worktree_merge",
                action
            )),
        }
    }

    pub async fn execute_async(args: &Value) -> Result<String> {
        Self::execute(args)
    }

    pub fn git_status() -> Result<String> {
        Self::git_status_in_dir(None)
    }

    pub fn git_status_in_dir(base_dir: Option<&Path>) -> Result<String> {
        let mut cmd = git_cmd();
        cmd.arg("status").arg("--short").arg("--branch");
        if let Some(d) = base_dir {
            cmd.current_dir(d);
        }
        let output = cmd.output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            return Err(anyhow::anyhow!("Git status failed: {}", stderr.trim()));
        }

        if stdout.trim().is_empty() {
            Ok("Clean working tree (no changes)".to_string())
        } else {
            Ok(format!("Git Status:\n{}", stdout.trim()))
        }
    }

    pub fn git_diff(staged: bool, file: Option<&str>) -> Result<String> {
        Self::git_diff_in_dir(staged, file, None)
    }

    pub fn git_diff_in_dir(staged: bool, file: Option<&str>, base_dir: Option<&Path>) -> Result<String> {
        let mut cmd = git_cmd();
        cmd.arg("diff");

        if staged {
            cmd.arg("--cached");
        }

        if let Some(f) = file {
            cmd.arg(f);
        }

        if let Some(d) = base_dir {
            cmd.current_dir(d);
        }

        let output = cmd.output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            return Err(anyhow::anyhow!("Git diff failed: {}", stderr.trim()));
        }

        if stdout.trim().is_empty() {
            Ok(if staged {
                "No staged changes."
            } else {
                "No unstaged changes."
            }
            .to_string())
        } else {
            Ok(stdout.to_string())
        }
    }

    pub fn git_log(count: usize) -> Result<String> {
        Self::git_log_in_dir(count, None)
    }

    pub fn git_log_in_dir(count: usize, base_dir: Option<&Path>) -> Result<String> {
        let mut cmd = git_cmd();
        cmd.arg("log")
            .arg(format!("-n{}", count))
            .arg("--oneline")
            .arg("--decorate");

        if let Some(d) = base_dir {
            cmd.current_dir(d);
        }

        let output = cmd.output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim().is_empty() {
            Ok("No commit history yet.".to_string())
        } else {
            Ok(format!("Recent Commits:\n{}", stdout.trim()))
        }
    }

    pub fn git_branch() -> Result<String> {
        Self::git_branch_in_dir(None)
    }

    pub fn git_branch_in_dir(base_dir: Option<&Path>) -> Result<String> {
        let mut cmd = git_cmd();
        cmd.arg("branch").arg("-a");

        if let Some(d) = base_dir {
            cmd.current_dir(d);
        }

        let output = cmd.output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim().is_empty() {
            Ok("No branches found.".to_string())
        } else {
            Ok(format!("Branches:\n{}", stdout.trim()))
        }
    }

    pub fn synthesize_commit_message() -> Result<String> {
        Self::synthesize_commit_message_in_dir(None)
    }

    pub fn synthesize_commit_message_in_dir(base_dir: Option<&Path>) -> Result<String> {
        let diff = Self::git_diff_in_dir(true, None, base_dir)?;
        let status = Self::git_status_in_dir(base_dir)?;

        if diff == "No staged changes." {
            return Ok("Cannot synthesize commit: No staged changes. Use `git add` first or inspect `git status`.".to_string());
        }

        // Analyze diff lines to provide structured conventional commit proposal
        let mut added_files = Vec::new();
        let mut modified_files = Vec::new();

        for line in status.lines() {
            if line.len() < 3 || line.starts_with("##") {
                continue;
            }
            let staged_code = line.chars().next().unwrap_or(' ');
            let file_path = line.get(3..).unwrap_or("").trim();

            match staged_code {
                'A' => added_files.push(file_path),
                'M' => modified_files.push(file_path),
                _ => {}
            }
        }

        let summary = if !added_files.is_empty() && modified_files.is_empty() {
            format!("feat: add {}", added_files.join(", "))
        } else if !modified_files.is_empty() && added_files.is_empty() {
            format!("refactor: update {}", modified_files.join(", "))
        } else {
            "feat: update workspace features and tools".to_string()
        };

        let added_str = if added_files.is_empty() {
            "none".to_string()
        } else {
            added_files.join(", ")
        };
        let modified_str = if modified_files.is_empty() {
            "none".to_string()
        } else {
            modified_files.join(", ")
        };

        Ok(format!(
            "--- Conventional Commit Proposal ---\nTitle: {}\n\nSummary:\nStaged Additions: {}\nStaged Modifications: {}\n\nDiff size: {} bytes",
            summary,
            added_str,
            modified_str,
            diff.len()
        ))
    }

    pub fn git_commit(message: &str) -> Result<String> {
        Self::git_commit_in_dir(message, None)
    }

    pub fn git_commit_in_dir(message: &str, base_dir: Option<&Path>) -> Result<String> {
        let trimmed = message.trim();
        if trimmed.is_empty() {
            return Err(anyhow::anyhow!("Commit message cannot be empty"));
        }

        let mut cmd = git_cmd();
        cmd.arg("commit")
            .arg("-m")
            .arg(trimmed);

        if let Some(d) = base_dir {
            cmd.current_dir(d);
        }

        let output = cmd.output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            return Err(anyhow::anyhow!("Git commit failed: {}", stderr.trim()));
        }

        Ok(format!("Commit successful:\n{}", stdout.trim()))
    }

    pub fn git_worktree_create(base_branch: &str, task_id: &str) -> Result<PathBuf> {
        git_worktree_create(base_branch, task_id)
    }

    pub fn git_worktree_remove(task_id: &str, force: bool) -> Result<()> {
        git_worktree_remove(task_id, force)
    }

    pub fn git_worktree_merge(task_id: &str, target_branch: &str) -> Result<String> {
        git_worktree_merge(task_id, target_branch)
    }

    pub fn git_worktree_list() -> Result<Vec<WorktreeInfo>> {
        git_worktree_list()
    }
}

pub fn git_worktree_create(base_branch: &str, task_id: &str) -> Result<PathBuf> {
    git_worktree_create_in_dir(base_branch, task_id, None)
}

pub fn git_worktree_create_at(
    base_branch: &str,
    branch_name: &str,
    target_path: &Path,
    base_dir: Option<&Path>,
) -> Result<PathBuf> {
    let repo_dir = match base_dir {
        Some(d) => d.to_path_buf(),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    };

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let target_path_str = target_path.to_string_lossy().to_string();
    let base = if base_branch.trim().is_empty() {
        "HEAD"
    } else {
        base_branch.trim()
    };

    let output = git_cmd()
        .current_dir(&repo_dir)
        .arg("worktree")
        .arg("add")
        .arg("-b")
        .arg(branch_name)
        .arg(&target_path_str)
        .arg(base)
        .output()?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        if target_path.exists() {
            let _ = fs::remove_dir_all(target_path);
        }
        return Err(anyhow::anyhow!(
            "Failed to create git worktree at '{}': {}",
            target_path_str,
            stderr.trim()
        ));
    }

    Ok(target_path.to_path_buf())
}

pub fn git_worktree_create_in_dir(
    base_branch: &str,
    task_id: &str,
    base_dir: Option<&Path>,
) -> Result<PathBuf> {
    let sanitized_id = task_id.trim().replace(['/', '\\', ' '], "-");
    if sanitized_id.is_empty() {
        return Err(anyhow::anyhow!("task_id cannot be empty"));
    }

    let repo_dir = match base_dir {
        Some(d) => d.to_path_buf(),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    };

    let pi_dir = repo_dir.join(".pi");
    let worktrees_parent = pi_dir.join("worktrees");
    fs::create_dir_all(&worktrees_parent)?;

    // Ensure .pi/.gitignore exists to automatically exclude worktrees from parent git tracking
    let pi_gitignore = pi_dir.join(".gitignore");
    if !pi_gitignore.exists() {
        let _ = fs::write(&pi_gitignore, "*\n!.gitignore\n");
    }

    let worktree_path = worktrees_parent.join(&sanitized_id);
    let branch_name = format!("pi-task-{}", sanitized_id);
    git_worktree_create_at(base_branch, &branch_name, &worktree_path, Some(&repo_dir))
}

pub fn git_worktree_remove(task_id: &str, force: bool) -> Result<()> {
    git_worktree_remove_in_dir(task_id, force, None)
}

pub fn git_worktree_remove_path(
    target_path: &Path,
    branch_name: Option<&str>,
    force: bool,
    base_dir: Option<&Path>,
) -> Result<()> {
    let repo_dir = match base_dir {
        Some(d) => d.to_path_buf(),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    };

    let target_path_str = target_path.to_string_lossy().to_string();

    let mut cmd = git_cmd();
    cmd.current_dir(&repo_dir).arg("worktree").arg("remove");
    if force {
        cmd.arg("--force");
    }
    cmd.arg(&target_path_str);

    let output = cmd.output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        if force && target_path.exists() {
            let _ = fs::remove_dir_all(target_path);
            let _ = git_cmd()
                .current_dir(&repo_dir)
                .args(["worktree", "prune"])
                .output();
        } else {
            return Err(anyhow::anyhow!(
                "Failed to remove git worktree at '{}': {}",
                target_path_str,
                stderr.trim()
            ));
        }
    }

    // Prune worktree metadata and delete branch if it exists
    let _ = git_cmd()
        .current_dir(&repo_dir)
        .args(["worktree", "prune"])
        .output();

    if let Some(branch) = branch_name {
        let _ = git_cmd()
            .current_dir(&repo_dir)
            .args(["branch", "-D", branch])
            .output();
    }

    Ok(())
}

pub fn git_worktree_remove_in_dir(
    task_id: &str,
    force: bool,
    base_dir: Option<&Path>,
) -> Result<()> {
    let sanitized_id = task_id.trim().replace(['/', '\\', ' '], "-");
    if sanitized_id.is_empty() {
        return Err(anyhow::anyhow!("task_id cannot be empty"));
    }

    let repo_dir = match base_dir {
        Some(d) => d.to_path_buf(),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    };

    let worktree_path = repo_dir.join(".pi").join("worktrees").join(&sanitized_id);
    let branch_name = format!("pi-task-{}", sanitized_id);
    git_worktree_remove_path(&worktree_path, Some(&branch_name), force, Some(&repo_dir))
}

pub fn git_worktree_list() -> Result<Vec<WorktreeInfo>> {
    git_worktree_list_in_dir(None)
}

pub fn git_worktree_list_in_dir(base_dir: Option<&Path>) -> Result<Vec<WorktreeInfo>> {
    let repo_dir = match base_dir {
        Some(d) => d.to_path_buf(),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    };

    let output = git_cmd()
        .current_dir(&repo_dir)
        .arg("worktree")
        .arg("list")
        .arg("--porcelain")
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "Failed to list git worktrees: {}",
            stderr.trim()
        ));
    }

    Ok(parse_porcelain_worktree_list(&stdout))
}

pub fn parse_porcelain_worktree_list(porcelain_output: &str) -> Vec<WorktreeInfo> {
    let mut list = Vec::new();
    let mut current_path: Option<String> = None;
    let mut current_head = String::new();
    let mut current_branch: Option<String> = None;
    let mut is_bare = false;
    let mut is_locked = false;
    let mut is_prunable = false;

    let flush_entry = |list: &mut Vec<WorktreeInfo>,
                       path: &mut Option<String>,
                       head: &mut String,
                       branch: &mut Option<String>,
                       bare: &mut bool,
                       locked: &mut bool,
                       prunable: &mut bool| {
        if let Some(p) = path.take() {
            list.push(WorktreeInfo {
                path: p,
                head: std::mem::take(head),
                branch: branch.take(),
                bare: *bare,
                locked: *locked,
                prunable: *prunable,
            });
            *bare = false;
            *locked = false;
            *prunable = false;
        }
    };

    for line in porcelain_output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            flush_entry(
                &mut list,
                &mut current_path,
                &mut current_head,
                &mut current_branch,
                &mut is_bare,
                &mut is_locked,
                &mut is_prunable,
            );
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("worktree ") {
            flush_entry(
                &mut list,
                &mut current_path,
                &mut current_head,
                &mut current_branch,
                &mut is_bare,
                &mut is_locked,
                &mut is_prunable,
            );
            current_path = Some(rest.trim().to_string());
        } else if let Some(rest) = trimmed.strip_prefix("HEAD ") {
            current_head = rest.trim().to_string();
        } else if let Some(rest) = trimmed.strip_prefix("branch ") {
            let b = rest.trim();
            let branch_clean = b.strip_prefix("refs/heads/").unwrap_or(b);
            current_branch = Some(branch_clean.to_string());
        } else if trimmed == "bare" {
            is_bare = true;
        } else if trimmed == "locked" || trimmed.starts_with("locked ") {
            is_locked = true;
        } else if trimmed == "prunable" || trimmed.starts_with("prunable ") {
            is_prunable = true;
        } else if trimmed == "detached" {
            current_branch = None;
        }
    }

    flush_entry(
        &mut list,
        &mut current_path,
        &mut current_head,
        &mut current_branch,
        &mut is_bare,
        &mut is_locked,
        &mut is_prunable,
    );

    list
}

pub fn git_worktree_merge(task_id: &str, target_branch: &str) -> Result<String> {
    git_worktree_merge_in_dir(task_id, target_branch, None)
}

pub fn git_merge_branch_in_dir(
    branch_to_merge: &str,
    target_branch: &str,
    base_dir: Option<&Path>,
) -> Result<String> {
    let repo_dir = match base_dir {
        Some(d) => d.to_path_buf(),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    };

    let target = if target_branch.trim().is_empty() {
        "HEAD"
    } else {
        target_branch.trim()
    };

    // 1. Checkout target branch in the base repo if specific branch name
    if target != "HEAD" {
        let checkout_out = git_cmd()
            .current_dir(&repo_dir)
            .args(["checkout", target])
            .output()?;

        if !checkout_out.status.success() {
            let stderr = String::from_utf8_lossy(&checkout_out.stderr);
            return Err(anyhow::anyhow!(
                "Failed to switch to target branch '{}': {}",
                target,
                stderr.trim()
            ));
        }
    }

    // 2. Perform merge
    let merge_out = git_cmd()
        .current_dir(&repo_dir)
        .args([
            "merge",
            "--no-ff",
            branch_to_merge,
            "-m",
            &format!("Merge branch '{}' into '{}'", branch_to_merge, target),
        ])
        .output()?;

    let stdout = String::from_utf8_lossy(&merge_out.stdout);
    let stderr = String::from_utf8_lossy(&merge_out.stderr);

    if !merge_out.status.success() {
        let conflict_files_out = git_cmd()
            .current_dir(&repo_dir)
            .args(["diff", "--name-only", "--diff-filter=U"])
            .output();

        let conflicted_files = conflict_files_out
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();

        let mut diag = format!(
            "Merge conflicts detected while merging '{}' into '{}'.\nGit Output:\n{}\n{}",
            branch_to_merge,
            target,
            stdout.trim(),
            stderr.trim()
        );

        if !conflicted_files.is_empty() {
            diag.push_str(&format!("\nConflicted Files:\n{}", conflicted_files));
        }

        // Abort merge to restore clean state in the base repository
        let _ = git_cmd()
            .current_dir(&repo_dir)
            .args(["merge", "--abort"])
            .output();

        return Err(anyhow::anyhow!(diag));
    }

    Ok(format!(
        "Successfully merged branch '{}' into '{}':\n{}",
        branch_to_merge,
        target,
        stdout.trim()
    ))
}

pub fn git_worktree_merge_in_dir(
    task_id: &str,
    target_branch: &str,
    base_dir: Option<&Path>,
) -> Result<String> {
    let sanitized_id = task_id.trim().replace(['/', '\\', ' '], "-");
    let branch_to_merge = format!("pi-task-{}", sanitized_id);
    let target = if target_branch.trim().is_empty() {
        "main"
    } else {
        target_branch
    };
    git_merge_branch_in_dir(&branch_to_merge, target, base_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_status_and_branch() {
        let status = GitTool::git_status();
        assert!(status.is_ok());

        let branch = GitTool::git_branch();
        assert!(branch.is_ok());
    }

    #[test]
    fn test_parse_porcelain_worktree_list() {
        let sample = r#"worktree /Users/pi/project
HEAD 1234567890abcdef1234567890abcdef12345678
branch refs/heads/main

worktree /Users/pi/project/.pi/worktrees/task-1
HEAD abcdef1234567890abcdef1234567890abcdef12
branch refs/heads/pi-task-task-1
locked
"#;
        let list = parse_porcelain_worktree_list(sample);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].path, "/Users/pi/project");
        assert_eq!(list[0].branch.as_deref(), Some("main"));
        assert!(!list[0].locked);

        assert_eq!(list[1].path, "/Users/pi/project/.pi/worktrees/task-1");
        assert_eq!(list[1].branch.as_deref(), Some("pi-task-task-1"));
        assert!(list[1].locked);
    }

    #[test]
    fn test_git_worktree_lifecycle_and_isolation() {
        let tmp = tempfile::tempdir().unwrap();
        let repo_path = tmp.path();

        // 1. Initialize temporary git repository
        let init_out = Command::new("git")
            .current_dir(repo_path)
            .args(["init", "-b", "main"])
            .output()
            .unwrap();
        assert!(init_out.status.success());

        // Configure git user for commits in test
        let _ = Command::new("git")
            .current_dir(repo_path)
            .args(["config", "user.name", "Pi Test"])
            .output();
        let _ = Command::new("git")
            .current_dir(repo_path)
            .args(["config", "user.email", "test@pi.rs"])
            .output();

        // Initial commit on main
        let readme_path = repo_path.join("README.md");
        fs::write(&readme_path, "# Main Branch\nInitial content\n").unwrap();

        let _ = Command::new("git").current_dir(repo_path).args(["add", "README.md"]).output();
        let _ = Command::new("git").current_dir(repo_path).args(["commit", "-m", "Initial commit"]).output();

        // 2. Create worktree for task-exp-1
        let wt_path = git_worktree_create_in_dir("main", "exp-1", Some(repo_path)).unwrap();
        assert!(wt_path.exists());
        assert!(repo_path.join(".pi").join(".gitignore").exists());

        // 3. Verify worktree list includes both main and task worktree
        let worktrees = git_worktree_list_in_dir(Some(repo_path)).unwrap();
        assert_eq!(worktrees.len(), 2);
        assert!(worktrees.iter().any(|w| w.branch.as_deref() == Some("pi-task-exp-1")));

        // 4. Modify and commit files strictly inside worktree (isolated)
        let wt_file = wt_path.join("feature.txt");
        fs::write(&wt_file, "Worktree feature content").unwrap();

        let _ = Command::new("git").current_dir(&wt_path).args(["add", "feature.txt"]).output();
        let commit_res = Command::new("git").current_dir(&wt_path).args(["commit", "-m", "Add feature in worktree"]).output().unwrap();
        assert!(commit_res.status.success());

        // 5. Verify isolation: feature.txt must NOT exist in main repo dir before merge
        assert!(!repo_path.join("feature.txt").exists());

        // 6. Merge worktree branch back into main
        let merge_res = git_worktree_merge_in_dir("exp-1", "main", Some(repo_path));
        assert!(merge_res.is_ok(), "Merge failed: {:?}", merge_res.err());

        // 7. Verify merged changes now exist on main
        assert!(repo_path.join("feature.txt").exists());
        assert_eq!(fs::read_to_string(repo_path.join("feature.txt")).unwrap(), "Worktree feature content");

        // 8. Remove worktree
        let remove_res = git_worktree_remove_in_dir("exp-1", true, Some(repo_path));
        assert!(remove_res.is_ok(), "Remove failed: {:?}", remove_res.err());

        // Verify worktree list no longer contains the removed worktree
        let worktrees_after = git_worktree_list_in_dir(Some(repo_path)).unwrap();
        assert_eq!(worktrees_after.len(), 1);
        assert_eq!(worktrees_after[0].branch.as_deref(), Some("main"));
    }

    #[test]
    fn test_git_worktree_merge_conflict_diagnostics() {
        let tmp = tempfile::tempdir().unwrap();
        let repo_path = tmp.path();

        let _ = Command::new("git").current_dir(repo_path).args(["init", "-b", "main"]).output();
        let _ = Command::new("git").current_dir(repo_path).args(["config", "user.name", "Pi Test"]).output();
        let _ = Command::new("git").current_dir(repo_path).args(["config", "user.email", "test@pi.rs"]).output();

        let file_path = repo_path.join("conflict.txt");
        fs::write(&file_path, "base line\n").unwrap();
        let _ = Command::new("git").current_dir(repo_path).args(["add", "conflict.txt"]).output();
        let _ = Command::new("git").current_dir(repo_path).args(["commit", "-m", "base commit"]).output();

        // Create worktree
        let wt_path = git_worktree_create_in_dir("main", "conflict-task", Some(repo_path)).unwrap();

        // Make change on main
        fs::write(&file_path, "main branch modification\n").unwrap();
        let _ = Command::new("git").current_dir(repo_path).args(["commit", "-am", "main mod"]).output();

        // Make conflicting change in worktree
        fs::write(wt_path.join("conflict.txt"), "worktree conflicting modification\n").unwrap();
        let _ = Command::new("git").current_dir(&wt_path).args(["commit", "-am", "worktree mod"]).output();

        // Merge should fail and provide conflict diagnostics
        let merge_res = git_worktree_merge_in_dir("conflict-task", "main", Some(repo_path));
        assert!(merge_res.is_err());
        let err_msg = merge_res.unwrap_err().to_string();
        assert!(err_msg.contains("Merge conflicts detected"));
        assert!(err_msg.contains("conflict.txt"));

        // Clean up
        let _ = git_worktree_remove_in_dir("conflict-task", true, Some(repo_path));
    }

    #[test]
    fn test_git_commit_proposal_and_diff_operations() {
        let tmp = tempfile::tempdir().unwrap();
        let repo_path = tmp.path();

        let _ = Command::new("git").current_dir(repo_path).args(["init", "-b", "main"]).output();
        let _ = Command::new("git").current_dir(repo_path).args(["config", "user.name", "Pi Test"]).output();
        let _ = Command::new("git").current_dir(repo_path).args(["config", "user.email", "test@pi.rs"]).output();

        let f1 = repo_path.join("file1.txt");
        fs::write(&f1, "Initial text\n").unwrap();
        let _ = Command::new("git").current_dir(repo_path).args(["add", "file1.txt"]).output();
        let _ = Command::new("git").current_dir(repo_path).args(["commit", "-m", "chore: initial"]).output();

        // Add a new file and stage it
        let f2 = repo_path.join("file2.txt");
        fs::write(&f2, "New file content\n").unwrap();
        let _ = Command::new("git").current_dir(repo_path).args(["add", "file2.txt"]).output();

        // Staged diff
        let diff_out = GitTool::git_diff_in_dir(true, None, Some(repo_path)).unwrap();
        assert!(!diff_out.is_empty());

        // Commit proposal
        let proposal = GitTool::synthesize_commit_message_in_dir(Some(repo_path)).unwrap();
        assert!(proposal.contains("feat: add") || proposal.contains("Conventional Commit Proposal"));

        // Commit in dir
        let commit_res = GitTool::git_commit_in_dir("feat: add file2.txt", Some(repo_path)).unwrap();
        assert!(commit_res.contains("Commit successful"));

        // Log in dir
        let log_out = GitTool::git_log_in_dir(5, Some(repo_path)).unwrap();
        assert!(log_out.contains("feat: add file2.txt"));
        assert!(log_out.contains("chore: initial"));
    }
}
