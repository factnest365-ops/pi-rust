use anyhow::Result;
use serde_json::Value;
use std::process::Command;

pub struct GithubTool;

impl GithubTool {
    pub fn execute(args: &Value) -> Result<String> {
        let action = args["action"]
            .as_str()
            .or_else(|| args["command"].as_str())
            .unwrap_or("pr_list");

        match action {
            "pr_list" => {
                let limit = args["limit"].as_u64().unwrap_or(10);
                Self::run_gh(&["pr", "list", "--limit", &limit.to_string()])
            }
            "pr_view" => {
                let pr_id = args["pr"]
                    .as_str()
                    .map(String::from)
                    .or_else(|| args["pr"].as_u64().map(|n| n.to_string()))
                    .or_else(|| args["number"].as_str().map(String::from))
                    .or_else(|| args["number"].as_u64().map(|n| n.to_string()))
                    .unwrap_or_default();
                if pr_id.is_empty() {
                    Self::run_gh(&["pr", "view"])
                } else {
                    Self::run_gh(&["pr", "view", &pr_id])
                }
            }
            "pr_diff" => {
                let pr_id = args["pr"]
                    .as_str()
                    .map(String::from)
                    .or_else(|| args["pr"].as_u64().map(|n| n.to_string()))
                    .or_else(|| args["number"].as_str().map(String::from))
                    .or_else(|| args["number"].as_u64().map(|n| n.to_string()))
                    .unwrap_or_default();
                if pr_id.is_empty() {
                    Self::run_gh(&["pr", "diff"])
                } else {
                    Self::run_gh(&["pr", "diff", &pr_id])
                }
            }
            "issue_list" => {
                let limit = args["limit"].as_u64().unwrap_or(10);
                Self::run_gh(&["issue", "list", "--limit", &limit.to_string()])
            }
            "issue_view" => {
                let issue_id = args["issue"]
                    .as_str()
                    .map(String::from)
                    .or_else(|| args["issue"].as_u64().map(|n| n.to_string()))
                    .or_else(|| args["number"].as_str().map(String::from))
                    .or_else(|| args["number"].as_u64().map(|n| n.to_string()))
                    .ok_or_else(|| anyhow::anyhow!("Missing 'issue' or 'number' argument"))?;
                Self::run_gh(&["issue", "view", &issue_id])
            }
            "run_list" => {
                let limit = args["limit"].as_u64().unwrap_or(10);
                Self::run_gh(&["run", "list", "--limit", &limit.to_string()])
            }
            _ => Err(anyhow::anyhow!(
                "Unknown github action '{}'. Supported actions: pr_list, pr_view, pr_diff, issue_list, issue_view, run_list",
                action
            )),
        }
    }

    fn run_gh(args: &[&str]) -> Result<String> {
        let output = Command::new("gh").args(args).output();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);

                if !out.status.success() {
                    if stderr.contains("not logged in") || stderr.contains("auth login") {
                        Ok("GitHub CLI (gh) is installed but not authenticated. Run `gh auth login` to authenticate.".to_string())
                    } else {
                        Err(anyhow::anyhow!("gh command failed: {}", stderr.trim()))
                    }
                } else if stdout.trim().is_empty() {
                    Ok("No output returned from GitHub CLI.".to_string())
                } else {
                    Ok(stdout.to_string())
                }
            }
            Err(_) => Ok("GitHub CLI (`gh`) is not installed on this system. Install via `brew install gh` or package manager.".to_string()),
        }
    }
}
