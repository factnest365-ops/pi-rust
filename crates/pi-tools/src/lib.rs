use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::process::Command;

pub mod ast;
pub mod crew;
pub mod git;
pub mod github;
pub mod lsp;
pub mod mcp;
pub mod plugins;
pub mod subagents;
pub mod web;

pub use ast::AstTool;
pub use crew::{
    register_crew_handler, CrewDispatchArgs, CrewMergeArgs, CrewStatusArgs, CrewToolHandler,
    CrewTools,
};
pub use git::{
    git_worktree_create, git_worktree_create_in_dir, git_worktree_list, git_worktree_list_in_dir,
    git_worktree_merge, git_worktree_merge_in_dir, git_worktree_remove, git_worktree_remove_in_dir,
    GitTool, WorktreeInfo,
};
pub use github::GithubTool;
pub use lsp::LspTool;
pub use mcp::{get_mcp_manager, McpManager, McpServerConfig, McpToolDefinition};
pub use plugins::ToolPlugin;
pub use subagents::{
    register_subagent_handler, InvokeSubagentArgs, ManageSubagentsArgs, SubagentToolHandler,
    SubagentTools,
};
pub use web::WebTool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub output: String,
    pub is_error: bool,
}

pub struct ToolExecutor;

impl ToolExecutor {
    pub async fn execute(call: &ToolCall) -> ToolResult {
        let res = match call.name.as_str() {
            "read" => Self::execute_read(&call.arguments),
            "write" => Self::execute_write(&call.arguments),
            "edit" => Self::execute_edit(&call.arguments),
            "bash" => Self::execute_bash_async(&call.arguments).await,
            "grep" => Self::execute_grep(&call.arguments),
            "find" => Self::execute_find(&call.arguments),
            "ls" => Self::execute_ls(&call.arguments),
            "web_fetch" | "web" => WebTool::execute_async(&call.arguments).await,
            "web_search" | "search" => WebTool::execute_search_async(&call.arguments).await,
            "git" => GitTool::execute(&call.arguments),
            "github" => GithubTool::execute(&call.arguments),
            "lsp" => LspTool::execute(&call.arguments),
            "ast" | "ast_slice" => AstTool::execute(&call.arguments),
            "invoke_subagent" => SubagentTools::execute_invoke_async(&call.arguments).await,
            "manage_subagents" => SubagentTools::execute_manage_async(&call.arguments).await,
            "crew_dispatch" => CrewTools::execute_dispatch_async(&call.arguments).await,
            "crew_status" => CrewTools::execute_status_async(&call.arguments).await,
            "crew_merge" => CrewTools::execute_merge_async(&call.arguments).await,
            mcp_tool_name => {
                let mcp_mgr = get_mcp_manager();
                let mgr = mcp_mgr.lock().await;
                if mgr.is_mcp_tool(mcp_tool_name) {
                    mgr.execute_tool(mcp_tool_name, &call.arguments).await
                } else {
                    Err(anyhow::anyhow!("Unknown tool: {}", call.name))
                }
            }
        };

        match res {
            Ok(output) => ToolResult {
                tool_call_id: call.id.clone(),
                output,
                is_error: false,
            },
            Err(err) => ToolResult {
                tool_call_id: call.id.clone(),
                output: format!("Tool Error: {}", err),
                is_error: true,
            },
        }
    }

    pub fn tool_definitions() -> Vec<serde_json::Value> {
        let mut defs = vec![
            serde_json::json!({
                "name": "read",
                "description": "View contents or line slices of a file",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Absolute or relative file path" },
                        "start_line": { "type": "integer", "description": "1-based starting line number" },
                        "end_line": { "type": "integer", "description": "1-based ending line number" }
                    },
                    "required": ["path"]
                }
            }),
            serde_json::json!({
                "name": "write",
                "description": "Create or overwrite a file with contents",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "File path to write" },
                        "content": { "type": "string", "description": "File content to write" }
                    },
                    "required": ["path", "content"]
                }
            }),
            serde_json::json!({
                "name": "edit",
                "description": "Make precise find-and-replace edits to an existing file",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "File path to edit" },
                        "target": { "type": "string", "description": "Exact text substring to replace" },
                        "replacement": { "type": "string", "description": "Replacement text" }
                    },
                    "required": ["path", "target", "replacement"]
                }
            }),
            serde_json::json!({
                "name": "bash",
                "description": "Execute shell commands to build, test, or inspect system state",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "command": { "type": "string", "description": "Shell command line to execute" }
                    },
                    "required": ["command"]
                }
            }),
            serde_json::json!({
                "name": "grep",
                "description": "Search directory files for a pattern or regular expression",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "pattern": { "type": "string", "description": "Search query pattern" },
                        "path": { "type": "string", "description": "Directory or file path to search" }
                    },
                    "required": ["pattern"]
                }
            }),
            serde_json::json!({
                "name": "find",
                "description": "Find files by filename or glob pattern",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "pattern": { "type": "string", "description": "Filename pattern to match" },
                        "path": { "type": "string", "description": "Directory to search within" }
                    },
                    "required": ["pattern"]
                }
            }),
            serde_json::json!({
                "name": "ls",
                "description": "List directory contents with file metadata and sizes",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Directory path to list" }
                    }
                }
            }),
            serde_json::json!({
                "name": "web_fetch",
                "description": "Fetch content from a URL via HTTP request and parse HTML into structured markdown",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "url": { "type": "string", "description": "HTTP/HTTPS URL to fetch" },
                        "max_length": { "type": "integer", "description": "Maximum character limit before truncation (default: 32000)" }
                    },
                    "required": ["url"]
                }
            }),
            serde_json::json!({
                "name": "web_search",
                "description": "Search the live web for current information, documentation, news, or technical questions",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Search query keywords" },
                        "max_results": { "type": "integer", "description": "Maximum number of search results to return (default: 5)" }
                    },
                    "required": ["query"]
                }
            }),
            serde_json::json!({
                "name": "git",
                "description": "Execute git inspection, diffing, status checks, commit synthesis, commit operations, or git worktree workspace isolation",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["status", "diff", "log", "branch", "commit_proposal", "commit", "worktree_add", "worktree_remove", "worktree_list", "worktree_merge"],
                            "description": "Git action to perform"
                        },
                        "staged": { "type": "boolean", "description": "When action is 'diff', diff only staged changes" },
                        "file": { "type": "string", "description": "Optional specific file path for diff" },
                        "count": { "type": "integer", "description": "When action is 'log', number of commits to show" },
                        "message": { "type": "string", "description": "Commit message when action is 'commit'" },
                        "task_id": { "type": "string", "description": "Task identifier for worktree creation, removal, or merge" },
                        "base_branch": { "type": "string", "description": "Base branch or ref when creating a worktree (default: HEAD)" },
                        "target_branch": { "type": "string", "description": "Target branch to merge worktree changes into (default: main)" },
                        "force": { "type": "boolean", "description": "Force removal of worktree even if modified" }
                    },
                    "required": ["action"]
                }
            }),
            serde_json::json!({
                "name": "github",
                "description": "Interact with GitHub PRs, issues, and workflow runs via GitHub CLI bridge",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["pr_list", "pr_view", "pr_diff", "issue_list", "issue_view", "run_list"],
                            "description": "GitHub action to perform"
                        },
                        "pr": { "type": "string", "description": "PR number or branch name" },
                        "issue": { "type": "string", "description": "Issue number" },
                        "limit": { "type": "integer", "description": "Max items to return (default: 10)" }
                    },
                    "required": ["action"]
                }
            }),
            serde_json::json!({
                "name": "lsp",
                "description": "Execute language server actions: diagnostics, document symbols, definition search, or hover docs",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["diagnostics", "symbols", "definition", "hover"],
                            "description": "LSP action to perform"
                        },
                        "path": { "type": "string", "description": "File or project path" },
                        "symbol": { "type": "string", "description": "Symbol name when action is definition or hover" }
                    },
                    "required": ["action"]
                }
            }),
            serde_json::json!({
                "name": "ast",
                "description": "Syntactically slice a function, struct, class, or outline file structure without guessing lines",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Source code file path" },
                        "symbol": { "type": "string", "description": "Optional symbol/function name to extract" }
                    },
                    "required": ["path"]
                }
            }),
            serde_json::json!({
                "name": "invoke_subagent",
                "description": "Spawn an autonomous background subagent with isolated context to perform a dedicated task",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "Descriptive name or role for the subagent" },
                        "task": { "type": "string", "description": "Detailed task instructions for the subagent" },
                        "model": { "type": "string", "description": "Optional model override for the subagent" },
                        "tools": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Optional list of allowed tool names for the subagent"
                        }
                    },
                    "required": ["name", "task"]
                }
            }),
            serde_json::json!({
                "name": "manage_subagents",
                "description": "Inspect, list, query status, or cancel active background subagents",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["list", "status", "kill"],
                            "description": "Action to perform on subagents"
                        },
                        "id": {
                            "type": "string",
                            "description": "Subagent ID (required for 'status' and 'kill' actions)"
                        }
                    },
                    "required": ["action"]
                }
            }),
            serde_json::json!({
                "name": "crew_dispatch",
                "description": "First Mate fleet tool: Dispatch a crew task (Ship or Scout) to an isolated git worktree or visible session",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "shape": {
                            "type": "string",
                            "enum": ["ship", "scout"],
                            "description": "Task shape: 'ship' for code modifications in isolated worktree, or 'scout' for read-only research/investigation"
                        },
                        "task": { "type": "string", "description": "Clear, detailed task specification for the crewmate" },
                        "mode": {
                            "type": "string",
                            "enum": ["local-only", "direct-pr", "no-mistakes"],
                            "description": "Merge authority mode (default: local-only)"
                        },
                        "backend": {
                            "type": "string",
                            "enum": ["herdr", "tmux", "worktree"],
                            "description": "Session multiplexer backend (default: herdr)"
                        },
                        "verify_cmd": { "type": "string", "description": "Optional validation command to run before merging (e.g. 'cargo test')" }
                    },
                    "required": ["task"]
                }
            }),
            serde_json::json!({
                "name": "crew_status",
                "description": "First Mate fleet tool: Query status and reconciliation of active fleet tasks",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "task_id": { "type": "string", "description": "Optional specific task ID to query" },
                        "action": {
                            "type": "string",
                            "enum": ["list", "inspect", "reconcile"],
                            "description": "Status action to perform (default: list)"
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "crew_merge",
                "description": "First Mate fleet tool: Review and merge a completed Ship task worktree back into target branch",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "task_id": { "type": "string", "description": "ID of the completed Ship task" },
                        "target_branch": { "type": "string", "description": "Branch to merge into (default: HEAD)" },
                        "verify_cmd": { "type": "string", "description": "Optional verification command to execute before merge" }
                    },
                    "required": ["task_id"]
                }
            })
        ];

        let mcp_mgr = get_mcp_manager();
        if let Ok(mgr) = mcp_mgr.try_lock() {
            defs.extend(mgr.get_tool_definitions());
        }

        defs
    }

    fn execute_read(args: &serde_json::Value) -> Result<String> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'path'"))?;

        let start_line = args["start_line"]
            .as_u64()
            .or_else(|| args["offset"].as_u64())
            .unwrap_or(1) as usize;

        let end_line = args["end_line"]
            .as_u64()
            .map(|v| v as usize)
            .or_else(|| {
                args["limit"]
                    .as_u64()
                    .map(|lim| start_line + (lim as usize).saturating_sub(1))
            })
            .unwrap_or(usize::MAX);

        if start_line == 0 {
            return Err(anyhow::anyhow!("'start_line' must be >= 1 (1-indexed)"));
        }

        if start_line > end_line {
            return Ok(String::new());
        }

        let file = File::open(path)?;
        let reader = BufReader::new(file);

        let mut output = String::new();
        for (idx, line_res) in reader.lines().enumerate() {
            let line_num = idx + 1;
            if line_num > end_line {
                break;
            }
            if line_num >= start_line {
                let line = line_res?;
                output.push_str(&format!("{:4} | {}\n", line_num, line));
            }
        }

        Ok(output)
    }

    fn execute_write(args: &serde_json::Value) -> Result<String> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'path'"))?;
        let content = args["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'content'"))?;

        if let Some(parent) = std::path::Path::new(path).parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }

        fs::write(path, content)?;
        Ok(format!("Successfully wrote {} bytes to {}", content.len(), path))
    }

    fn execute_edit(args: &serde_json::Value) -> Result<String> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'path'"))?;
        let target = args["target"]
            .as_str()
            .or_else(|| args["oldText"].as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'target' or 'oldText'"))?;
        let replacement = args["replacement"]
            .as_str()
            .or_else(|| args["newText"].as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'replacement' or 'newText'"))?;

        let content = fs::read_to_string(path)?;
        let mut occurrences = content.matches(target).count();
        let mut actual_target = target.to_string();
        let mut actual_replacement = replacement.to_string();

        // Line-ending normalization fallback if CRLF vs LF mismatch
        if occurrences == 0 && target.contains('\n') {
            if !target.contains("\r\n") && content.contains("\r\n") {
                let crlf_target = target.replace('\n', "\r\n");
                let crlf_occ = content.matches(&crlf_target).count();
                if crlf_occ > 0 {
                    occurrences = crlf_occ;
                    actual_target = crlf_target;
                    actual_replacement = replacement.replace('\n', "\r\n");
                }
            } else if target.contains("\r\n") && !content.contains("\r\n") {
                let lf_target = target.replace("\r\n", "\n");
                let lf_occ = content.matches(&lf_target).count();
                if lf_occ > 0 {
                    occurrences = lf_occ;
                    actual_target = lf_target;
                    actual_replacement = replacement.replace("\r\n", "\n");
                }
            }
        }

        if occurrences == 0 {
            return Err(anyhow::anyhow!("Target string not found in {}", path));
        }
        if occurrences > 1 {
            return Err(anyhow::anyhow!(
                "Target string occurs {} times in {}. Provide more surrounding lines to disambiguate the edit.",
                occurrences,
                path
            ));
        }

        let new_content = content.replacen(&actual_target, &actual_replacement, 1);
        fs::write(path, new_content)?;
        Ok(format!("Successfully edited {}", path))
    }

    async fn execute_bash_async(args: &serde_json::Value) -> Result<String> {
        let command_str = args["command"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'command'"))?;

        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c").arg(command_str);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        if let Some(cwd) = args["cwd"].as_str() {
            cmd.current_dir(cwd);
        }

        let mut child = cmd.spawn()?;
        let timeout_dur = std::time::Duration::from_secs(120);

        let mut stdout_pipe = child.stdout.take();
        let mut stderr_pipe = child.stderr.take();

        let wait_future = async {
            let mut stdout_bytes = Vec::new();
            let mut stderr_bytes = Vec::new();

            if let Some(ref mut s) = stdout_pipe {
                use tokio::io::AsyncReadExt;
                let _ = s.read_to_end(&mut stdout_bytes).await;
            }
            if let Some(ref mut s) = stderr_pipe {
                use tokio::io::AsyncReadExt;
                let _ = s.read_to_end(&mut stderr_bytes).await;
            }

            let _status = child.wait().await?;
            Ok::<_, anyhow::Error>((stdout_bytes, stderr_bytes))
        };

        match tokio::time::timeout(timeout_dur, wait_future).await {
            Ok(Ok((stdout_bytes, stderr_bytes))) => {
                let stdout = String::from_utf8_lossy(&stdout_bytes);
                let stderr = String::from_utf8_lossy(&stderr_bytes);

                let mut res = String::new();
                if !stdout.is_empty() {
                    res.push_str(&stdout);
                }
                if !stderr.is_empty() {
                    if !res.is_empty() {
                        res.push_str("\n--- STDERR ---\n");
                    }
                    res.push_str(&stderr);
                }
                Ok(res)
            }
            Ok(Err(e)) => Err(anyhow::anyhow!("Subprocess execution error: {}", e)),
            Err(_) => {
                let _ = child.kill().await;
                Err(anyhow::anyhow!("Command execution timed out after 120 seconds"))
            }
        }
    }

    fn execute_grep(args: &serde_json::Value) -> Result<String> {
        let pattern = args["pattern"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'pattern'"))?;
        let search_path = args["path"].as_str().unwrap_or(".");

        let output = Command::new("grep")
            .arg("-rnI")
            .arg("-e")
            .arg(pattern)
            .arg(search_path)
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.is_empty() {
            Ok("No matches found.".to_string())
        } else {
            Ok(stdout.to_string())
        }
    }

    fn execute_find(args: &serde_json::Value) -> Result<String> {
        let pattern = args["pattern"].as_str().unwrap_or("*");
        let search_path = args["path"].as_str().unwrap_or(".");

        let match_flag = if pattern.contains('/') { "-path" } else { "-name" };

        let output = Command::new("find")
            .arg(search_path)
            .arg(match_flag)
            .arg(pattern)
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.is_empty() {
            Ok("No files found matching pattern.".to_string())
        } else {
            Ok(stdout.to_string())
        }
    }

    fn execute_ls(args: &serde_json::Value) -> Result<String> {
        let dir_path = args["path"].as_str().unwrap_or(".");
        let entries = fs::read_dir(dir_path)?;

        let mut items = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name();
            let is_dir = entry.metadata().map(|m| m.is_dir()).unwrap_or(false);
            let len = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let name_str = name.to_string_lossy().to_string();
            items.push((is_dir, name_str, len));
        }

        // Sort directories first, then alphabetical by name
        items.sort_by(|a, b| {
            b.0.cmp(&a.0).then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase()))
        });

        let mut output = String::new();
        for (is_dir, name, len) in items {
            let kind = if is_dir { "DIR " } else { "FILE" };
            output.push_str(&format!("{} | {} ({} bytes)\n", kind, name, len));
        }

        if output.is_empty() {
            Ok("(empty directory)".to_string())
        } else {
            Ok(output)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_write_and_read_tool() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("test_file.txt");
        let path_str = file_path.to_str().unwrap();

        let write_call = ToolCall {
            id: "call-1".to_string(),
            name: "write".to_string(),
            arguments: serde_json::json!({
                "path": path_str,
                "content": "line 1\nline 2\nline 3\nline 4\nline 5\n"
            }),
        };
        let write_res = ToolExecutor::execute(&write_call).await;
        assert!(!write_res.is_error);

        // Read specific line slice
        let read_call = ToolCall {
            id: "call-2".to_string(),
            name: "read".to_string(),
            arguments: serde_json::json!({
                "path": path_str,
                "start_line": 2,
                "end_line": 4
            }),
        };
        let read_res = ToolExecutor::execute(&read_call).await;
        assert!(!read_res.is_error);
        assert!(read_res.output.contains("   2 | line 2"));
        assert!(read_res.output.contains("   4 | line 4"));
        assert!(!read_res.output.contains("line 1"));
        assert!(!read_res.output.contains("line 5"));
    }

    #[tokio::test]
    async fn test_read_boundary_cases() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("test_boundary.txt");
        let path_str = file_path.to_str().unwrap();

        fs::write(&file_path, "one\ntwo\n").unwrap();

        // Inverted lines should not panic
        let inverted_call = ToolCall {
            id: "call-inv".to_string(),
            name: "read".to_string(),
            arguments: serde_json::json!({
                "path": path_str,
                "start_line": 10,
                "end_line": 2
            }),
        };
        let res = ToolExecutor::execute(&inverted_call).await;
        assert!(!res.is_error);
        assert_eq!(res.output, "");
    }

    #[tokio::test]
    async fn test_edit_tool() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("edit_me.txt");
        let path_str = file_path.to_str().unwrap();

        fs::write(&file_path, "Hello world!").unwrap();

        let edit_call = ToolCall {
            id: "call-edit".to_string(),
            name: "edit".to_string(),
            arguments: serde_json::json!({
                "path": path_str,
                "target": "world",
                "replacement": "Pi Rust"
            }),
        };
        let res = ToolExecutor::execute(&edit_call).await;
        assert!(!res.is_error);

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Hello Pi Rust!");
    }

    #[tokio::test]
    async fn test_edit_tool_ambiguous_target() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("ambiguous.txt");
        let path_str = file_path.to_str().unwrap();

        fs::write(&file_path, "dup line\ndup line\nthird line").unwrap();

        let edit_call = ToolCall {
            id: "call-edit-dup".to_string(),
            name: "edit".to_string(),
            arguments: serde_json::json!({
                "path": path_str,
                "target": "dup line",
                "replacement": "new line"
            }),
        };
        let res = ToolExecutor::execute(&edit_call).await;
        assert!(res.is_error);
        assert!(res.output.contains("occurs 2 times"));
    }

    #[tokio::test]
    async fn test_grep_flag_hyphen_pattern() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("flags.txt");
        fs::write(&file_path, "arg --all-targets is supported\nother text").unwrap();

        let grep_call = ToolCall {
            id: "call-grep".to_string(),
            name: "grep".to_string(),
            arguments: serde_json::json!({
                "pattern": "--all-targets",
                "path": tmp.path().to_str().unwrap()
            }),
        };
        let res = ToolExecutor::execute(&grep_call).await;
        assert!(!res.is_error);
        assert!(res.output.contains("--all-targets"));
    }

    #[tokio::test]
    async fn test_bash_cwd_and_execution() {
        let tmp = tempfile::tempdir().unwrap();
        let bash_call = ToolCall {
            id: "call-bash".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "pwd",
                "cwd": tmp.path().to_str().unwrap()
            }),
        };
        let res = ToolExecutor::execute(&bash_call).await;
        assert!(!res.is_error);
        assert!(res.output.contains(tmp.path().file_name().unwrap().to_str().unwrap()));
    }

    #[tokio::test]
    async fn test_git_tool_execution() {
        let git_call = ToolCall {
            id: "call-git".to_string(),
            name: "git".to_string(),
            arguments: serde_json::json!({ "action": "status" }),
        };
        let res = ToolExecutor::execute(&git_call).await;
        assert!(!res.is_error);
        assert!(res.output.contains("Git Status:") || res.output.contains("Clean working tree"));
    }

    #[tokio::test]
    async fn test_web_tool_execution() {
        let mock_server = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = mock_server.local_addr().unwrap();
        let url = format!("http://{}", addr);

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = mock_server.accept().await {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut buf = [0u8; 1024];
                let _ = socket.read(&mut buf).await;
                let body = "<html><body><h1>Hello Web</h1><p>From Pi Rust</p></body></html>";
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = socket.write_all(resp.as_bytes()).await;
            }
        });

        let web_call = ToolCall {
            id: "call-web".to_string(),
            name: "web_fetch".to_string(),
            arguments: serde_json::json!({ "url": url }),
        };
        let res = ToolExecutor::execute(&web_call).await;
        assert!(!res.is_error);
        assert!(res.output.contains("# Hello Web"));
        assert!(res.output.contains("From Pi Rust"));
    }

    #[tokio::test]
    async fn test_lsp_tool_execution() {
        let lsp_call = ToolCall {
            id: "call-lsp".to_string(),
            name: "lsp".to_string(),
            arguments: serde_json::json!({ "action": "diagnostics" }),
        };
        let res = ToolExecutor::execute(&lsp_call).await;
        assert!(!res.is_error);
        assert!(res.output.contains("Diagnostics:"));
    }

    #[tokio::test]
    async fn test_read_offset_limit() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("lines.txt");
        let file_str = file_path.to_str().unwrap();

        fs::write(&file_path, "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n").unwrap();

        let call = ToolCall {
            id: "call-read-lim".to_string(),
            name: "read".to_string(),
            arguments: serde_json::json!({
                "path": file_str,
                "offset": 2,
                "limit": 2
            }),
        };
        let res = ToolExecutor::execute(&call).await;
        assert!(!res.is_error);
        assert!(res.output.contains("Line 2"));
        assert!(res.output.contains("Line 3"));
        assert!(!res.output.contains("Line 1"));
        assert!(!res.output.contains("Line 4"));
    }

    #[tokio::test]
    async fn test_edit_crlf_normalization() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("crlf.txt");
        let file_str = file_path.to_str().unwrap();

        // Write CRLF file
        fs::write(&file_path, "header\r\nold_line_1\r\nold_line_2\r\nfooter\r\n").unwrap();

        // Edit with LF target
        let call = ToolCall {
            id: "call-edit-crlf".to_string(),
            name: "edit".to_string(),
            arguments: serde_json::json!({
                "path": file_str,
                "target": "old_line_1\nold_line_2",
                "replacement": "new_line_1\nnew_line_2"
            }),
        };
        let res = ToolExecutor::execute(&call).await;
        assert!(!res.is_error);

        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("new_line_1"));
        assert!(content.contains("new_line_2"));
        assert!(!content.contains("old_line_1"));
    }

    #[tokio::test]
    async fn test_ast_tool_execution() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("code.rs");
        let file_str = file_path.to_str().unwrap();

        fs::write(&file_path, "pub fn compute_sum(a: i32, b: i32) -> i32 {\n    a + b\n}\n").unwrap();

        let ast_call = ToolCall {
            id: "call-ast".to_string(),
            name: "ast".to_string(),
            arguments: serde_json::json!({
                "path": file_str,
                "symbol": "compute_sum"
            }),
        };
        let res = ToolExecutor::execute(&ast_call).await;
        assert!(!res.is_error);
        assert!(res.output.contains("pub fn compute_sum"));
        assert!(res.output.contains("a + b"));
    }

    #[tokio::test]
    async fn test_git_worktree_tool_executor() {
        let tmp = tempfile::tempdir().unwrap();
        let repo_path = tmp.path();

        let _ = Command::new("git").current_dir(repo_path).args(["init", "-b", "main"]).output();
        let _ = Command::new("git").current_dir(repo_path).args(["config", "user.name", "Pi Test"]).output();
        let _ = Command::new("git").current_dir(repo_path).args(["config", "user.email", "test@pi.rs"]).output();

        let readme = repo_path.join("README.md");
        fs::write(&readme, "# Init\n").unwrap();
        let _ = Command::new("git").current_dir(repo_path).args(["add", "README.md"]).output();
        let _ = Command::new("git").current_dir(repo_path).args(["commit", "-m", "Initial commit"]).output();

        // 1. Create worktree via direct helper
        let wt = git_worktree_create_in_dir("main", "wt-exec-1", Some(repo_path)).unwrap();
        assert!(wt.exists());

        // 2. List worktrees via direct helper
        let list = git_worktree_list_in_dir(Some(repo_path)).unwrap();
        assert_eq!(list.len(), 2);

        // 3. Remove worktree via direct helper
        let rem = git_worktree_remove_in_dir("wt-exec-1", true, Some(repo_path));
        assert!(rem.is_ok());

        // 4. Verify list via tool call in current workspace
        let call = ToolCall {
            id: "call-wt-list".to_string(),
            name: "git".to_string(),
            arguments: serde_json::json!({ "action": "worktree_list" }),
        };
        let res = ToolExecutor::execute(&call).await;
        assert!(!res.is_error);
        assert!(res.output.contains("Active Git Worktrees") || res.output.contains("No active worktrees"));
    }
}
