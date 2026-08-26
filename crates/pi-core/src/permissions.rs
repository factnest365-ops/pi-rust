use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

use pi_tools::ToolCall;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    ReadOnly,
    Mutation,
    Exec,
    Network,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Decision {
    Allow,
    Deny(String),
    Ask,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub tool_name: String,
    pub risk: RiskLevel,
    pub decision: Decision,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionPolicy {
    pub default_readonly: Decision,
    pub default_mutation: Decision,
    pub default_exec: Decision,
    pub default_network: Decision,
    pub tool_overrides: Vec<ToolOverride>,
    pub protected_globs: Vec<String>,
    pub command_deny_patterns: Vec<String>,
    pub expand_home: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOverride {
    pub tool_name: String,
    pub decision: Decision,
}

impl Default for PermissionPolicy {
    fn default() -> Self {
        Self {
            default_readonly: Decision::Allow,
            default_mutation: Decision::Allow,
            default_exec: Decision::Ask,
            default_network: Decision::Allow,
            tool_overrides: vec![],
            protected_globs: vec![
                ".git/**".to_string(),
                ".env".to_string(),
                "~/.tau/**".to_string(),
            ],
            command_deny_patterns: vec![
                "rm -rf /".to_string(),
                "git push --force".to_string(),
                "git push -f".to_string(),
                "sudo".to_string(),
            ],
            expand_home: true,
        }
    }
}

pub trait ApprovalSink: Send + Sync {
    fn request_approval(&self, tool: &str, args: &Value, risk: RiskLevel) -> bool;
}

#[derive(Clone)]
pub struct PermissionBridge {
    pub policy: PermissionPolicy,
    sink: Option<Arc<dyn ApprovalSink + Send + Sync>>,
    pub audit_log: Vec<AuditEntry>,
}

impl Default for PermissionBridge {
    fn default() -> Self {
        Self::new(PermissionPolicy::default())
    }
}

impl PermissionBridge {
    pub fn new(policy: PermissionPolicy) -> Self {
        Self {
            policy,
            sink: None,
            audit_log: Vec::new(),
        }
    }

    pub fn with_sink(mut self, sink: Arc<dyn ApprovalSink + Send + Sync>) -> Self {
        self.sink = Some(sink);
        self
    }

    pub fn check(&mut self, call: &ToolCall) -> Result<(), String> {
        let decision = self.evaluate(call);
        let reason = match &decision {
            Decision::Allow => None,
            Decision::Deny(reason) => Some(reason.clone()),
            Decision::Ask => Some("Approval required".to_string()),
        };

        self.audit_log.push(AuditEntry {
            tool_name: call.name.clone(),
            risk: classify(&call.name, &call.arguments),
            decision: decision.clone(),
            reason,
        });

        match decision {
            Decision::Allow => Ok(()),
            Decision::Deny(reason) => Err(reason),
            Decision::Ask => {
                if let Some(ref sink) = self.sink {
                    if sink.request_approval(
                        &call.name,
                        &call.arguments,
                        classify(&call.name, &call.arguments),
                    ) {
                        Ok(())
                    } else {
                        Err("Approval denied".to_string())
                    }
                } else {
                    Err(
                        "Permission denied: approval required but no sink is configured"
                            .to_string(),
                    )
                }
            }
        }
    }

    fn evaluate(&self, call: &ToolCall) -> Decision {
        let tool = call.name.as_str();

        if let Some(override_decision) = tool_override_decision(&self.policy, tool) {
            return override_decision;
        }

        match classify(tool, &call.arguments) {
            RiskLevel::ReadOnly => self.policy.default_readonly.clone(),
            RiskLevel::Mutation => {
                if tool_matches_protected_glob(call, &self.policy) {
                    return Decision::Deny(
                        "Protected path blocked by permission policy".to_string(),
                    );
                }
                self.policy.default_mutation.clone()
            }
            RiskLevel::Exec => {
                if tool_matches_command_deny(call, &self.policy) {
                    return Decision::Deny("Command blocked by permission policy".to_string());
                }
                self.policy.default_exec.clone()
            }
            RiskLevel::Network => self.policy.default_network.clone(),
        }
    }
}

fn tool_override_decision(policy: &PermissionPolicy, tool: &str) -> Option<Decision> {
    policy
        .tool_overrides
        .iter()
        .find(|o| o.tool_name == tool)
        .map(|o| o.decision.clone())
}

fn tool_matches_protected_glob(call: &ToolCall, policy: &PermissionPolicy) -> bool {
    let path = call
        .arguments
        .get("path")
        .and_then(|v| v.as_str())
        .or_else(|| call.arguments.get("file").and_then(|v| v.as_str()))
        .or_else(|| call.arguments.get("filepath").and_then(|v| v.as_str()))
        .or_else(|| call.arguments.get("file_path").and_then(|v| v.as_str()));

    if let Some(path) = path {
        return matches_any_glob(path, &policy.protected_globs, policy.expand_home);
    }

    false
}

fn matches_any_glob(path: &str, globs: &[String], expand_home: bool) -> bool {
    let normalized = normalize_path(path, expand_home);
    globs.iter().any(|glob| matches_glob(&normalized, glob))
}

fn matches_glob(path: &str, glob: &str) -> bool {
    if glob == "**" {
        return true;
    }
    if let Some(suffix) = glob.strip_prefix("**/") {
        return path.ends_with(suffix)
            || path.contains(&format!("/{}", suffix))
            || path == suffix.strip_prefix('/').unwrap_or(suffix);
    }
    if glob.ends_with("/**") {
        let prefix = glob.trim_end_matches("/**");
        // Match the prefix as a complete path-prefix at a segment boundary:
        // `.git/**` must block `/repo/.git/config` and `.git/config`, not
        // only paths that literally start with the prefix characters.
        return path == prefix
            || path.starts_with(&format!("{}/", prefix))
            || path.contains(&format!("/{}/", prefix));
    }
    if glob.contains("**") {
        return simple_glob_eq(path, glob);
    }

    if glob.starts_with("~/")
        && let Ok(expanded) = std::env::var("HOME")
    {
        let expanded_glob = format!("{}{}", expanded, &glob[1..]);
        return simple_glob_eq(path, &expanded_glob);
    }

    // A glob with no slash is a bare filename: protect it at any path depth
    // (`.env` blocks `/repo/.env` and `~/.env`, not just a literal `.env`).
    if !glob.contains('/') {
        return path == glob || path.ends_with(&format!("/{}", glob));
    }

    simple_glob_eq(path, glob)
}

fn simple_glob_eq(path: &str, pattern: &str) -> bool {
    if pattern == path {
        return true;
    }
    let segments_p = split_glob_segments(path);
    let segments_g = split_glob_segments(pattern);
    if segments_p.len() != segments_g.len() {
        return false;
    }
    segments_p
        .iter()
        .zip(segments_g.iter())
        .all(|(p, g)| *g == "**" || p == g)
}

fn split_glob_segments(value: &str) -> Vec<&str> {
    value
        .split('/')
        .map(|s| s.trim_start_matches("~/"))
        .collect()
}

fn tool_matches_command_deny(call: &ToolCall, policy: &PermissionPolicy) -> bool {
    let tool = call.name.as_str();
    let command = if matches!(tool, "bash" | "sh" | "shell" | "exec") {
        call.arguments.get("command").and_then(|v| v.as_str())
    } else {
        None
    };

    if let Some(command) = command {
        let normalized = normalize_command(command);
        for pattern in &policy.command_deny_patterns {
            if normalize_command(pattern) == normalized {
                return true;
            }
            if command_matches_pattern(&normalized, pattern) {
                return true;
            }
        }
    }

    false
}

fn normalize_command(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn command_matches_pattern(command: &str, pattern: &str) -> bool {
    if command == pattern {
        return true;
    }
    let command_lower = command.to_ascii_lowercase();
    let pattern_lower = pattern.to_ascii_lowercase();
    if command_lower.contains(&pattern_lower) {
        return true;
    }
    if let Some(index) = command_lower.find(&pattern_lower) {
        let before = command_lower[..index].trim_end();
        if before.is_empty()
            || before.ends_with(' ')
            || before.ends_with(';')
            || before.ends_with('&')
            || before.ends_with('|')
        {
            return true;
        }
    }
    false
}

pub fn classify(tool_name: &str, _arguments: &Value) -> RiskLevel {
    match tool_name {
        "read" | "grep" | "find" | "ls" | "list_dir" | "dir" => RiskLevel::ReadOnly,
        "write" | "edit" => RiskLevel::Mutation,
        "git" => RiskLevel::Mutation,
        "bash" | "sh" | "shell" | "exec" | "lsp" => RiskLevel::Exec,
        "web_fetch" | "web" | "fetch" | "web_search" | "search" => RiskLevel::Network,
        "github" | "gh" => RiskLevel::Network,
        "ast" | "ast_slice" => RiskLevel::ReadOnly,
        "invoke_subagent" | "manage_subagents" => RiskLevel::Exec,
        "crew_dispatch" | "crew_status" | "crew_merge" => RiskLevel::Exec,
        "speculate" | "speculative_race" => RiskLevel::Exec,
        _ => {
            if tool_name.starts_with("mcp_") {
                RiskLevel::Network
            } else {
                RiskLevel::Exec
            }
        }
    }
}

pub fn normalize_path(path: &str, expand_home: bool) -> String {
    if expand_home
        && path.starts_with("~/")
        && let Ok(home) = std::env::var("HOME")
    {
        return format!("{}{}", home.trim_end_matches('/'), &path[1..]);
    }
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct FakeSink {
        approved: Mutex<Option<bool>>,
    }

    impl ApprovalSink for FakeSink {
        fn request_approval(&self, _tool: &str, _args: &Value, _risk: RiskLevel) -> bool {
            *self.approved.lock().unwrap() == Some(true)
        }
    }

    #[test]
    fn classify_readonly_tools() {
        for tool in [
            "read",
            "grep",
            "find",
            "ls",
            "list_dir",
            "dir",
            "ast",
            "ast_slice",
        ] {
            assert_eq!(
                classify(tool, &Value::Null),
                RiskLevel::ReadOnly,
                "tool={}",
                tool
            );
        }
    }

    #[test]
    fn classify_mutation_tools() {
        for tool in ["write", "edit", "git"] {
            assert_eq!(
                classify(tool, &Value::Null),
                RiskLevel::Mutation,
                "tool={}",
                tool
            );
        }
    }

    #[test]
    fn classify_exec_and_network_tools() {
        assert_eq!(classify("bash", &Value::Null), RiskLevel::Exec);
        assert_eq!(classify("lsp", &Value::Null), RiskLevel::Exec);
        assert_eq!(classify("web_fetch", &Value::Null), RiskLevel::Network);
        assert_eq!(classify("mcp_custom", &Value::Null), RiskLevel::Network);
    }

    #[test]
    fn command_deny_blocks_force_push_and_rm_rf_root() {
        let mut bridge = PermissionBridge::default();

        let bad1 = ToolCall {
            id: "c1".into(),
            name: "bash".into(),
            arguments: serde_json::json!({"command": "git push --force"}),
        };
        let bad2 = ToolCall {
            id: "c2".into(),
            name: "bash".into(),
            arguments: serde_json::json!({"command": "rm -rf /"}),
        };
        let bad3 = ToolCall {
            id: "c3".into(),
            name: "bash".into(),
            arguments: serde_json::json!({"command": "sudo apt update"}),
        };

        assert!(matches!(bridge.check(&bad1), Err(reason) if reason.contains("blocked")));
        assert!(matches!(bridge.check(&bad2), Err(reason) if reason.contains("blocked")));
        assert!(matches!(bridge.check(&bad3), Err(reason) if reason.contains("blocked")));
    }

    #[test]
    fn protected_path_glob_blocks_mutation_on_protected_path() {
        let mut bridge = PermissionBridge::default();

        let write_git = ToolCall {
            id: "cw".into(),
            name: "write".into(),
            arguments: serde_json::json!({"path": "/repo/.git/config", "content": "x"}),
        };
        let edit_env = ToolCall {
            id: "ce".into(),
            name: "edit".into(),
            arguments: serde_json::json!({"path": "/repo/.env", "target": "x", "replacement": "y"}),
        };

        assert!(
            matches!(bridge.check(&write_git), Err(reason) if reason.contains("Protected path blocked"))
        );
        assert!(
            matches!(bridge.check(&edit_env), Err(reason) if reason.contains("Protected path blocked"))
        );
    }

    #[test]
    fn ask_without_sink_denies_with_clear_reason() {
        let mut bridge = PermissionBridge::default();
        let call = ToolCall {
            id: "ca".into(),
            name: "bash".into(),
            arguments: serde_json::json!({"command": "echo hi"}),
        };
        let err = bridge.check(&call).unwrap_err();
        assert_eq!(
            err,
            "Permission denied: approval required but no sink is configured"
        );
    }

    #[test]
    fn ask_with_sink_uses_approval_result() {
        let approved = Arc::new(FakeSink {
            approved: Mutex::new(Some(true)),
        });
        let denied = Arc::new(FakeSink {
            approved: Mutex::new(Some(false)),
        });

        let mut allow_bridge = PermissionBridge::default().with_sink(approved);
        let mut deny_bridge = PermissionBridge::default().with_sink(denied);

        let allow = ToolCall {
            id: "c1".into(),
            name: "bash".into(),
            arguments: serde_json::json!({"command": "echo hi"}),
        };
        let deny = ToolCall {
            id: "c2".into(),
            name: "bash".into(),
            arguments: serde_json::json!({"command": "echo bye"}),
        };

        assert!(allow_bridge.check(&allow).is_ok());
        assert!(matches!(deny_bridge.check(&deny), Err(reason) if reason == "Approval denied"));
    }

    #[test]
    fn audit_log_records_decision_and_includes_allows() {
        let mut bridge = PermissionBridge::default();
        let read = ToolCall {
            id: "cr".into(),
            name: "read".into(),
            arguments: serde_json::json!({"path": "README.md"}),
        };
        bridge.check(&read).unwrap();
        let write = ToolCall {
            id: "cw".into(),
            name: "write".into(),
            arguments: serde_json::json!({"path": "README.md", "content": "x"}),
        };
        let _ = bridge.check(&write);

        assert_eq!(bridge.audit_log.len(), 2);
        assert_eq!(bridge.audit_log[0].tool_name, "read");
        assert!(matches!(bridge.audit_log[0].decision, Decision::Allow));
        assert_eq!(bridge.audit_log[1].tool_name, "write");
        assert!(matches!(bridge.audit_log[1].decision, Decision::Allow));
    }
}
