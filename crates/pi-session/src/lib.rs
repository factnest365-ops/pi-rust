use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    System,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub children_ids: Vec<String>,
    pub role: Role,
    pub content: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrajectoryStep {
    pub step_index: usize,
    pub node_id: String,
    pub parent_id: Option<String>,
    pub role: Role,
    pub content: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<serde_json::Value>,
    pub token_estimate: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrajectoryExport {
    pub session_id: String,
    pub branch_head_id: String,
    pub total_steps: usize,
    pub total_estimated_tokens: usize,
    pub steps: Vec<TrajectoryStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BranchDiff {
    pub session_id: String,
    pub lca_node_id: Option<String>,
    pub node_a: String,
    pub node_b: String,
    pub branch_a_divergent: Vec<SessionNode>,
    pub branch_b_divergent: Vec<SessionNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionHeader {
    pub r#type: String,
    pub version: u32,
    pub id: String,
    pub timestamp: String,
    pub cwd: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTree {
    pub session_id: String,
    pub root_id: String,
    pub active_node_id: String,
    pub nodes: HashMap<String, SessionNode>,
    pub disk_path: Option<PathBuf>,
}

impl Default for SessionTree {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionTree {
    pub fn new() -> Self {
        let session_id = Uuid::new_v4().to_string();
        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string());
        let disk_path = Self::compute_disk_path(&cwd, &session_id);
        Self::new_with_disk_path(Some(disk_path), &cwd, session_id)
    }

    pub fn new_with_disk_path(disk_path: Option<PathBuf>, cwd: &str, session_id: String) -> Self {
        let root_id = Uuid::new_v4().to_string()[..8].to_string();
        let timestamp = chrono::Utc::now().to_rfc3339();

        let root_node = SessionNode {
            id: root_id.clone(),
            parent_id: None,
            children_ids: Vec::new(),
            role: Role::System,
            content: "Session initialized".to_string(),
            timestamp: timestamp.clone(),
            tool_call_id: None,
            tool_name: None,
            tool_calls: None,
        };

        let mut nodes = HashMap::new();
        nodes.insert(root_id.clone(), root_node.clone());

        let tree = Self {
            session_id,
            root_id: root_id.clone(),
            active_node_id: root_id,
            nodes,
            disk_path,
        };

        tree.init_disk_file(cwd, &timestamp);
        tree
    }

    fn compute_disk_path(cwd: &str, session_id: &str) -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let encoded_cwd = cwd.replace(['/', '\\', ':'], "-");
        let dir = home.join(".pi").join("agent").join("sessions").join(format!("--{}--", encoded_cwd.trim_matches('-')));
        let _ = fs::create_dir_all(&dir);
        dir.join(format!("{}.jsonl", session_id))
    }

    fn init_disk_file(&self, cwd: &str, timestamp: &str) {
        if let Some(ref path) = self.disk_path {
            let header = SessionHeader {
                r#type: "session".to_string(),
                version: 3,
                id: self.session_id.clone(),
                timestamp: timestamp.to_string(),
                cwd: cwd.to_string(),
            };
            if let Ok(json) = serde_json::to_string(&header) {
                let mut content = format!("{}\n", json);
                if let Some(root_node) = self.nodes.get(&self.root_id) {
                    let root_payload = serde_json::json!({
                        "type": "message",
                        "id": root_node.id,
                        "parentId": root_node.parent_id,
                        "timestamp": root_node.timestamp,
                        "role": root_node.role,
                        "content": root_node.content,
                    });
                    if let Ok(root_json) = serde_json::to_string(&root_payload) {
                        content.push_str(&format!("{}\n", root_json));
                    }
                }
                let _ = fs::write(path, content);
            }
        }
    }

    pub fn load_from_jsonl(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let mut lines = content.lines();

        let header_line = lines.next().ok_or_else(|| anyhow::anyhow!("Empty session file"))?;
        let header: SessionHeader = serde_json::from_str(header_line)?;

        let mut nodes: HashMap<String, SessionNode> = HashMap::new();
        let mut root_id = String::new();
        let mut last_node_id = String::new();

        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                let id = val.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if id.is_empty() {
                    continue;
                }
                let parent_id = val.get("parentId").or_else(|| val.get("parent_id")).and_then(|v| v.as_str()).map(ToString::to_string);
                let timestamp = val.get("timestamp").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let content = val.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let role: Role = if let Some(r) = val.get("role") {
                    serde_json::from_value(r.clone()).unwrap_or(Role::System)
                } else {
                    Role::System
                };
                let tool_call_id = val.get("toolCallId").or_else(|| val.get("tool_call_id")).and_then(|v| v.as_str()).map(ToString::to_string);
                let tool_name = val.get("toolName").or_else(|| val.get("tool_name")).and_then(|v| v.as_str()).map(ToString::to_string);
                let tool_calls = val.get("toolCalls").or_else(|| val.get("tool_calls")).cloned();

                if root_id.is_empty() {
                    root_id = id.clone();
                }

                let node = SessionNode {
                    id: id.clone(),
                    parent_id: parent_id.clone(),
                    children_ids: Vec::new(),
                    role,
                    content,
                    timestamp,
                    tool_call_id,
                    tool_name,
                    tool_calls,
                };

                if let Some(ref pid) = parent_id
                    && let Some(p_node) = nodes.get_mut(pid)
                {
                    p_node.children_ids.push(id.clone());
                }

                nodes.insert(id.clone(), node);
                last_node_id = id;
            }
        }

        if root_id.is_empty() {
            return Err(anyhow::anyhow!("No session nodes found in {}", path.display()));
        }

        Ok(Self {
            session_id: header.id,
            root_id: root_id.clone(),
            active_node_id: if last_node_id.is_empty() { root_id } else { last_node_id },
            nodes,
            disk_path: Some(path.to_path_buf()),
        })
    }

    pub fn append_child(&mut self, role: Role, content: String) -> String {
        self.append_child_with_metadata(role, content, None, None, None)
    }

    pub fn append_child_with_metadata(
        &mut self,
        role: Role,
        content: String,
        tool_call_id: Option<String>,
        tool_name: Option<String>,
        tool_calls: Option<serde_json::Value>,
    ) -> String {
        let new_id = Uuid::new_v4().to_string()[..8].to_string();
        let timestamp = chrono::Utc::now().to_rfc3339();
        let parent_id = Some(self.active_node_id.clone());

        let new_node = SessionNode {
            id: new_id.clone(),
            parent_id: parent_id.clone(),
            children_ids: Vec::new(),
            role,
            content,
            timestamp,
            tool_call_id,
            tool_name,
            tool_calls,
        };

        if let Some(parent) = self.nodes.get_mut(&self.active_node_id) {
            parent.children_ids.push(new_id.clone());
        }

        self.nodes.insert(new_id.clone(), new_node.clone());
        self.active_node_id = new_id.clone();

        // Write entry to JSONL file on disk
        if let Some(ref path) = self.disk_path {
            let mut entry_payload = serde_json::json!({
                "type": "message",
                "id": new_node.id,
                "parentId": new_node.parent_id,
                "timestamp": new_node.timestamp,
                "role": new_node.role,
                "content": new_node.content,
            });
            if let Some(ref tcid) = new_node.tool_call_id {
                entry_payload["toolCallId"] = serde_json::Value::String(tcid.clone());
            }
            if let Some(ref tname) = new_node.tool_name {
                entry_payload["toolName"] = serde_json::Value::String(tname.clone());
            }
            if let Some(ref tcalls) = new_node.tool_calls {
                entry_payload["toolCalls"] = tcalls.clone();
            }

            if let (Ok(entry_json), Ok(mut file)) = (
                serde_json::to_string(&entry_payload),
                OpenOptions::new().create(true).append(true).open(path),
            ) {
                let _ = writeln!(file, "{}", entry_json);
            }
        }

        new_id
    }

    pub fn rewind_to(&mut self, node_id: &str) -> bool {
        if self.nodes.contains_key(node_id) {
            self.active_node_id = node_id.to_string();
            true
        } else {
            false
        }
    }

    pub fn get_branch_history(&self, head_node_id: &str) -> Vec<&SessionNode> {
        let mut history = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut current_id = Some(head_node_id.to_string());

        while let Some(id) = current_id {
            if !visited.insert(id.clone()) {
                // Circular reference safeguard
                break;
            }
            if let Some(node) = self.nodes.get(&id) {
                history.push(node);
                current_id = node.parent_id.clone();
            } else {
                break;
            }
        }

        history.reverse();
        history
    }

    pub fn get_active_branch_history(&self) -> Vec<&SessionNode> {
        self.get_branch_history(&self.active_node_id)
    }

    pub fn simulate_rewind_to(&self, target_node_id: &str) -> Vec<&SessionNode> {
        if self.nodes.contains_key(target_node_id) {
            self.get_branch_history(target_node_id)
        } else {
            Vec::new()
        }
    }

    pub fn export_trajectory(&self, branch_node_id: Option<&str>) -> TrajectoryExport {
        let head_id = branch_node_id.unwrap_or(&self.active_node_id);
        let history = self.get_branch_history(head_id);

        let mut steps = Vec::with_capacity(history.len());
        let mut total_estimated_tokens = 0;

        for (idx, node) in history.iter().enumerate() {
            let mut est = node.content.len().div_ceil(4);
            if let Some(ref tc) = node.tool_calls {
                est += tc.to_string().len().div_ceil(4);
            }
            if est == 0 {
                est = 1;
            }
            total_estimated_tokens += est;

            steps.push(TrajectoryStep {
                step_index: idx,
                node_id: node.id.clone(),
                parent_id: node.parent_id.clone(),
                role: node.role.clone(),
                content: node.content.clone(),
                timestamp: node.timestamp.clone(),
                tool_call_id: node.tool_call_id.clone(),
                tool_name: node.tool_name.clone(),
                tool_calls: node.tool_calls.clone(),
                token_estimate: est,
            });
        }

        TrajectoryExport {
            session_id: self.session_id.clone(),
            branch_head_id: head_id.to_string(),
            total_steps: steps.len(),
            total_estimated_tokens,
            steps,
        }
    }

    pub fn diff_branches(&self, node_a: &str, node_b: &str) -> BranchDiff {
        let path_a = self.get_branch_history(node_a);
        let path_b = self.get_branch_history(node_b);

        // Find Lowest Common Ancestor (longest common prefix)
        let mut lca_idx = 0;
        while lca_idx < path_a.len() && lca_idx < path_b.len() && path_a[lca_idx].id == path_b[lca_idx].id {
            lca_idx += 1;
        }

        let lca_node_id = if lca_idx > 0 {
            Some(path_a[lca_idx - 1].id.clone())
        } else {
            None
        };

        let branch_a_divergent = if lca_idx < path_a.len() {
            path_a[lca_idx..].iter().map(|n| (*n).clone()).collect()
        } else {
            Vec::new()
        };

        let branch_b_divergent = if lca_idx < path_b.len() {
            path_b[lca_idx..].iter().map(|n| (*n).clone()).collect()
        } else {
            Vec::new()
        };

        BranchDiff {
            session_id: self.session_id.clone(),
            lca_node_id,
            node_a: node_a.to_string(),
            node_b: node_b.to_string(),
            branch_a_divergent,
            branch_b_divergent,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tree_append_and_history() {
        let mut tree = SessionTree::new();
        let root_id = tree.root_id.clone();
        assert_eq!(tree.active_node_id, root_id);

        let user_node_id = tree.append_child(Role::User, "Hello".to_string());
        let assistant_node_id = tree.append_child(Role::Assistant, "Hi there!".to_string());

        let history = tree.get_active_branch_history();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].id, root_id);
        assert_eq!(history[1].id, user_node_id);
        assert_eq!(history[2].id, assistant_node_id);
    }

    #[test]
    fn test_tree_rewind_and_fork() {
        let mut tree = SessionTree::new();
        let n1 = tree.append_child(Role::User, "Step 1".to_string());
        let _n2 = tree.append_child(Role::Assistant, "Response 1".to_string());

        assert!(tree.rewind_to(&n1));
        assert_eq!(tree.active_node_id, n1);

        let n3 = tree.append_child(Role::User, "Step 1 Forked".to_string());
        let history = tree.get_active_branch_history();

        assert_eq!(history.len(), 3);
        assert_eq!(history[1].id, n1);
        assert_eq!(history[2].id, n3);
    }

    #[test]
    fn test_rewind_invalid_node() {
        let mut tree = SessionTree::new();
        assert!(!tree.rewind_to("non_existent_id"));
    }

    #[test]
    fn test_load_from_jsonl() {
        let tmp = tempfile::tempdir().unwrap();
        let session_file = tmp.path().join("test_session.jsonl");

        let mut tree = SessionTree::new_with_disk_path(Some(session_file.clone()), ".", "test-session-123".to_string());
        let _ = tree.append_child(Role::User, "Hello from disk".to_string());
        let _ = tree.append_child(Role::Assistant, "Response from disk".to_string());

        let loaded = SessionTree::load_from_jsonl(&session_file).unwrap();
        assert_eq!(loaded.session_id, "test-session-123");
        let history = loaded.get_active_branch_history();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].role, Role::System);
        assert_eq!(history[1].content, "Hello from disk");
        assert_eq!(history[2].content, "Response from disk");
    }

    #[test]
    fn test_circular_reference_safeguard() {
        let mut tree = SessionTree::new();
        let root = tree.root_id.clone();
        let n1 = tree.append_child(Role::User, "Step 1".to_string());

        // Create artificial cycle: root's parent becomes n1
        if let Some(r_node) = tree.nodes.get_mut(&root) {
            r_node.parent_id = Some(n1.clone());
        }

        let history = tree.get_active_branch_history();
        assert!(history.len() <= 2);
    }

    #[test]
    fn test_malformed_jsonl_handling() {
        let tmp = tempfile::tempdir().unwrap();
        let session_file = tmp.path().join("corrupted.jsonl");
        fs::write(
            &session_file,
            r#"{"type":"session","version":3,"id":"corrupt-1","timestamp":"2026-08-15T00:00:00Z","cwd":"."}
{"type":"unknown","id":""}
{"type":"message","id":"msg1","timestamp":"2026-08-15T00:00:01Z","role":"user","content":"Valid"}
"#,
        )
        .unwrap();

        let loaded = SessionTree::load_from_jsonl(&session_file).unwrap();
        let history = loaded.get_active_branch_history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].content, "Valid");
    }

    #[test]
    fn test_export_trajectory() {
        let mut tree = SessionTree::new();
        let root_id = tree.root_id.clone();
        let u1 = tree.append_child(Role::User, "Hello trajectory".to_string());
        let a1 = tree.append_child_with_metadata(
            Role::Assistant,
            "Calling tool".to_string(),
            None,
            None,
            Some(serde_json::json!([{
                "id": "tc1",
                "type": "function",
                "function": { "name": "bash", "arguments": "{\"command\":\"echo 1\"}" }
            }])),
        );
        let t1 = tree.append_child_with_metadata(
            Role::Tool,
            "1\n".to_string(),
            Some("tc1".to_string()),
            Some("bash".to_string()),
            None,
        );

        let export = tree.export_trajectory(None);
        assert_eq!(export.session_id, tree.session_id);
        assert_eq!(export.branch_head_id, t1);
        assert_eq!(export.total_steps, 4);
        assert_eq!(export.steps.len(), 4);
        assert_eq!(export.steps[0].node_id, root_id);
        assert_eq!(export.steps[1].node_id, u1);
        assert_eq!(export.steps[2].node_id, a1);
        assert_eq!(export.steps[3].node_id, t1);
        assert_eq!(export.steps[3].tool_name.as_deref(), Some("bash"));
        assert!(export.total_estimated_tokens > 0);

        // Test JSON serialization of export
        let json = serde_json::to_string(&export).unwrap();
        let deserialized: TrajectoryExport = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.total_steps, 4);
        assert_eq!(deserialized.steps[3].content, "1\n");
    }

    #[test]
    fn test_simulate_rewind_to() {
        let mut tree = SessionTree::new();
        let root_id = tree.root_id.clone();
        let u1 = tree.append_child(Role::User, "Step 1".to_string());
        let a1 = tree.append_child(Role::Assistant, "Response 1".to_string());
        let u2 = tree.append_child(Role::User, "Step 2".to_string());

        assert_eq!(tree.active_node_id, u2);

        let sim = tree.simulate_rewind_to(&a1);
        assert_eq!(sim.len(), 3);
        assert_eq!(sim[0].id, root_id);
        assert_eq!(sim[1].id, u1);
        assert_eq!(sim[2].id, a1);

        // State remains unmutated
        assert_eq!(tree.active_node_id, u2);

        // Non-existent node returns empty
        let non_existent = tree.simulate_rewind_to("bogus_id");
        assert!(non_existent.is_empty());
    }

    #[test]
    fn test_diff_branches_and_lca() {
        let mut tree = SessionTree::new();
        let root_id = tree.root_id.clone();

        // Common turn: root -> u1
        let u1 = tree.append_child(Role::User, "Common question".to_string());

        // Branch A: u1 -> a1_a -> u2_a
        let a1_a = tree.append_child(Role::Assistant, "Branch A answer".to_string());
        let u2_a = tree.append_child(Role::User, "Branch A followup".to_string());

        // Rewind to u1 and create Branch B: u1 -> a1_b -> u2_b -> a2_b
        tree.rewind_to(&u1);
        let a1_b = tree.append_child(Role::Assistant, "Branch B answer".to_string());
        let u2_b = tree.append_child(Role::User, "Branch B followup".to_string());
        let a2_b = tree.append_child(Role::Assistant, "Branch B final".to_string());

        let diff = tree.diff_branches(&u2_a, &a2_b);
        assert_eq!(diff.lca_node_id, Some(u1.clone()));
        assert_eq!(diff.node_a, u2_a);
        assert_eq!(diff.node_b, a2_b);

        assert_eq!(diff.branch_a_divergent.len(), 2);
        assert_eq!(diff.branch_a_divergent[0].id, a1_a);
        assert_eq!(diff.branch_a_divergent[1].id, u2_a);

        assert_eq!(diff.branch_b_divergent.len(), 3);
        assert_eq!(diff.branch_b_divergent[0].id, a1_b);
        assert_eq!(diff.branch_b_divergent[1].id, u2_b);
        assert_eq!(diff.branch_b_divergent[2].id, a2_b);

        // Test identical nodes (no divergence)
        let same_diff = tree.diff_branches(&u2_a, &u2_a);
        assert_eq!(same_diff.lca_node_id, Some(u2_a));
        assert!(same_diff.branch_a_divergent.is_empty());
        assert!(same_diff.branch_b_divergent.is_empty());

        // Test diff with root
        let root_diff = tree.diff_branches(&root_id, &u1);
        assert_eq!(root_diff.lca_node_id, Some(root_id));
        assert!(root_diff.branch_a_divergent.is_empty());
        assert_eq!(root_diff.branch_b_divergent.len(), 1);
        assert_eq!(root_diff.branch_b_divergent[0].id, u1);
    }
}
