use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    User,
    Assistant,
    System,
    Tool,
}

impl Role {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System => "system",
            Role::Tool => "tool",
        }
    }
}

impl Serialize for Role {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Role {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct RoleVisitor;

        impl serde::de::Visitor<'_> for RoleVisitor {
            type Value = Role;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a role string (user, assistant, system, tool)")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(match v.trim().to_lowercase().as_str() {
                    "user" => Role::User,
                    "assistant" => Role::Assistant,
                    "tool" => Role::Tool,
                    "system" => Role::System,
                    _ => Role::System,
                })
            }
        }

        deserializer.deserialize_str(RoleVisitor)
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for Role {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.trim().to_lowercase().as_str() {
            "user" => Role::User,
            "assistant" => Role::Assistant,
            "tool" => Role::Tool,
            "system" => Role::System,
            _ => Role::System,
        })
    }
}

impl AsRef<str> for Role {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionNode {
    pub id: String,
    #[serde(default, alias = "parentId")]
    pub parent_id: Option<String>,
    #[serde(default, alias = "childrenIds")]
    pub children_ids: Vec<String>,
    pub role: Role,
    pub content: String,
    pub timestamp: String,
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "toolCallId")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "toolName")]
    pub tool_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "toolCalls")]
    pub tool_calls: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrajectoryStep {
    pub step_index: usize,
    pub node_id: String,
    #[serde(default, alias = "parentId")]
    pub parent_id: Option<String>,
    pub role: Role,
    pub content: String,
    pub timestamp: String,
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "toolCallId")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "toolName")]
    pub tool_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "toolCalls")]
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
    #[serde(default = "default_session_type")]
    pub r#type: String,
    #[serde(default = "default_version")]
    pub version: u32,
    pub id: String,
    #[serde(default)]
    pub timestamp: String,
    #[serde(default)]
    pub cwd: String,
}

fn default_session_type() -> String {
    "session".to_string()
}

fn default_version() -> u32 {
    3
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
    /// Generate a 12-character lowercase hex node ID.
    pub fn generate_node_id() -> String {
        let raw = Uuid::new_v4().to_string().replace('-', "");
        raw[..12].to_string()
    }

    pub fn new() -> Self {
        let session_id = Uuid::new_v4().to_string();
        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string());
        let disk_path = Self::compute_disk_path(&cwd, &session_id);
        Self::new_with_disk_path(Some(disk_path), &cwd, session_id)
    }

    pub fn new_with_disk_path(disk_path: Option<PathBuf>, cwd: &str, session_id: String) -> Self {
        let root_id = Self::generate_node_id();
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
        let dir = home
            .join(".pi")
            .join("agent")
            .join("sessions")
            .join(format!("--{}--", encoded_cwd.trim_matches('-')));
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

    /// Loads and reconstructs a session tree from a JSONL file with resilience against
    /// corrupted, truncated, or out-of-order lines.
    pub fn load_from_jsonl(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        if content.trim().is_empty() {
            return Err(anyhow::anyhow!("Session file is empty: {}", path.display()));
        }

        let mut session_id = String::new();
        let mut nodes: HashMap<String, SessionNode> = HashMap::new();
        let mut node_order: Vec<String> = Vec::new();
        let mut root_id = String::new();
        let mut last_node_id = String::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Attempt to parse JSON object - safely skip malformed or truncated lines
            let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) else {
                continue;
            };

            // Check if this line is a session header
            let line_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if line_type == "session" || line_type == "header" {
                if let Some(sid) = val.get("id").and_then(|v| v.as_str())
                    && !sid.is_empty()
                {
                    session_id = sid.to_string();
                }
                continue;
            }

            // Extract node ID
            let id = val
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if id.is_empty() {
                continue;
            }

            let parent_id = val
                .get("parentId")
                .or_else(|| val.get("parent_id"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(ToString::to_string);
            let timestamp = val
                .get("timestamp")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let content_str = val
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let role: Role = if let Some(r) = val.get("role") {
                if let Some(s) = r.as_str() {
                    match s.trim().to_lowercase().as_str() {
                        "user" => Role::User,
                        "assistant" => Role::Assistant,
                        "tool" => Role::Tool,
                        "system" => Role::System,
                        _ => serde_json::from_value(r.clone()).unwrap_or(Role::System),
                    }
                } else {
                    serde_json::from_value(r.clone()).unwrap_or(Role::System)
                }
            } else {
                Role::System
            };

            let tool_call_id = val
                .get("toolCallId")
                .or_else(|| val.get("tool_call_id"))
                .and_then(|v| v.as_str())
                .map(ToString::to_string);
            let tool_name = val
                .get("toolName")
                .or_else(|| val.get("tool_name"))
                .and_then(|v| v.as_str())
                .map(ToString::to_string);
            let tool_calls = val
                .get("toolCalls")
                .or_else(|| val.get("tool_calls"))
                .cloned();

            if root_id.is_empty() && parent_id.is_none() {
                root_id = id.clone();
            }

            let node = SessionNode {
                id: id.clone(),
                parent_id,
                children_ids: Vec::new(),
                role,
                content: content_str,
                timestamp,
                tool_call_id,
                tool_name,
                tool_calls,
            };

            nodes.insert(id.clone(), node);
            node_order.push(id.clone());
            last_node_id = id;
        }

        if nodes.is_empty() {
            return Err(anyhow::anyhow!(
                "No valid session nodes found in {}",
                path.display()
            ));
        }

        if root_id.is_empty() {
            root_id = node_order.first().cloned().unwrap_or_default();
        }

        if session_id.is_empty() {
            session_id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(ToString::to_string)
                .unwrap_or_else(|| Uuid::new_v4().to_string());
        }

        // Reconstruct complete children_ids relationships across all loaded nodes in chronological order
        let mut parent_to_children: HashMap<String, Vec<String>> = HashMap::new();
        for node_id in &node_order {
            if let Some(node) = nodes.get(node_id)
                && let Some(ref pid) = node.parent_id
            {
                parent_to_children
                    .entry(pid.clone())
                    .or_default()
                    .push(node_id.clone());
            }
        }
        for (pid, children) in parent_to_children {
            if let Some(p_node) = nodes.get_mut(&pid) {
                p_node.children_ids = children;
            }
        }

        let active_node_id = if last_node_id.is_empty() {
            root_id.clone()
        } else {
            last_node_id
        };

        Ok(Self {
            session_id,
            root_id,
            active_node_id,
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
        let mut new_id = Self::generate_node_id();
        while self.nodes.contains_key(&new_id) {
            new_id = Self::generate_node_id();
        }
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

    /// Rewinds the active pointer to `node_id`. Returns true if node exists.
    pub fn rewind_to(&mut self, node_id: &str) -> bool {
        if self.nodes.contains_key(node_id) {
            self.active_node_id = node_id.to_string();
            true
        } else {
            false
        }
    }

    /// Rewinds to `target_node_id` preparing a new branch fork point.
    pub fn fork_from(&mut self, target_node_id: &str) -> bool {
        self.rewind_to(target_node_id)
    }

    /// Rewinds to `target_node_id` and appends a child node, creating a new branch point.
    pub fn fork_from_with_message(
        &mut self,
        target_node_id: &str,
        role: Role,
        content: String,
    ) -> Option<String> {
        if self.rewind_to(target_node_id) {
            Some(self.append_child(role, content))
        } else {
            None
        }
    }

    /// Returns the linear root-to-leaf path leading up to `head_node_id`.
    /// Protected with a circular reference cycle detector.
    pub fn get_branch_history(&self, head_node_id: &str) -> Vec<&SessionNode> {
        let mut history = Vec::new();
        let mut visited = HashSet::new();
        let mut current_id = Some(head_node_id.to_string());

        while let Some(id) = current_id {
            if !visited.insert(id.clone()) {
                // Circular reference cycle safeguard
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
                role: node.role,
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

    /// Computes the Lowest Common Ancestor (LCA) and divergent branches between two nodes.
    pub fn diff_branches(&self, node_a: &str, node_b: &str) -> BranchDiff {
        let path_a = self.get_branch_history(node_a);
        let path_b = self.get_branch_history(node_b);

        // Find Lowest Common Ancestor (longest common prefix)
        let mut lca_idx = 0;
        while lca_idx < path_a.len()
            && lca_idx < path_b.len()
            && path_a[lca_idx].id == path_b[lca_idx].id
        {
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

    /// Finds the Lowest Common Ancestor (LCA) ID of two nodes.
    pub fn find_lca(&self, node_a: &str, node_b: &str) -> Option<String> {
        self.diff_branches(node_a, node_b).lca_node_id
    }

    /// Direct lookup of a session node.
    pub fn get_node(&self, node_id: &str) -> Option<&SessionNode> {
        self.nodes.get(node_id)
    }

    /// Mutable lookup of a session node.
    pub fn get_node_mut(&mut self, node_id: &str) -> Option<&mut SessionNode> {
        self.nodes.get_mut(node_id)
    }

    /// Checks if a node exists in the DAG.
    pub fn contains_node(&self, node_id: &str) -> bool {
        self.nodes.contains_key(node_id)
    }

    /// Gets immediate child nodes of a given node.
    pub fn get_children(&self, node_id: &str) -> Vec<&SessionNode> {
        if let Some(node) = self.nodes.get(node_id) {
            node.children_ids
                .iter()
                .filter_map(|cid| self.nodes.get(cid))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Gets all leaf nodes (nodes with no children) representing branch tips.
    pub fn get_leaf_nodes(&self) -> Vec<&SessionNode> {
        self.nodes
            .values()
            .filter(|n| n.children_ids.is_empty())
            .collect()
    }

    /// Total number of active branch tips in the DAG.
    pub fn branch_count(&self) -> usize {
        self.get_leaf_nodes().len()
    }

    /// Total number of nodes in the session DAG.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Calculates the maximum depth from root to any leaf in the DAG.
    pub fn tree_depth(&self) -> usize {
        let leaves = self.get_leaf_nodes();
        leaves
            .into_iter()
            .map(|leaf| self.get_branch_history(&leaf.id).len())
            .max()
            .unwrap_or(0)
    }

    /// Returns true if `ancestor_id` is an ancestor of `descendant_id`.
    pub fn is_ancestor_of(&self, ancestor_id: &str, descendant_id: &str) -> bool {
        let history = self.get_branch_history(descendant_id);
        history.iter().any(|n| n.id == ancestor_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_serialization_lowercase() {
        assert_eq!(serde_json::to_string(&Role::User).unwrap(), "\"user\"");
        assert_eq!(
            serde_json::to_string(&Role::Assistant).unwrap(),
            "\"assistant\""
        );
        assert_eq!(serde_json::to_string(&Role::System).unwrap(), "\"system\"");
        assert_eq!(serde_json::to_string(&Role::Tool).unwrap(), "\"tool\"");

        assert_eq!(Role::User.to_string(), "user");
        assert_eq!(Role::Assistant.to_string(), "assistant");
        assert_eq!(Role::System.to_string(), "system");
        assert_eq!(Role::Tool.to_string(), "tool");
    }

    #[test]
    fn test_role_deserialization_case_insensitive_and_fallback() {
        // Lowercase
        assert_eq!(
            serde_json::from_str::<Role>("\"user\"").unwrap(),
            Role::User
        );
        assert_eq!(
            serde_json::from_str::<Role>("\"assistant\"").unwrap(),
            Role::Assistant
        );
        assert_eq!(
            serde_json::from_str::<Role>("\"system\"").unwrap(),
            Role::System
        );
        assert_eq!(
            serde_json::from_str::<Role>("\"tool\"").unwrap(),
            Role::Tool
        );

        // Uppercase & Mixed Case
        assert_eq!(
            serde_json::from_str::<Role>("\"USER\"").unwrap(),
            Role::User
        );
        assert_eq!(
            serde_json::from_str::<Role>("\"User\"").unwrap(),
            Role::User
        );
        assert_eq!(
            serde_json::from_str::<Role>("\"ASSISTANT\"").unwrap(),
            Role::Assistant
        );
        assert_eq!(
            serde_json::from_str::<Role>("\"Assistant\"").unwrap(),
            Role::Assistant
        );
        assert_eq!(
            serde_json::from_str::<Role>("\"SYSTEM\"").unwrap(),
            Role::System
        );
        assert_eq!(
            serde_json::from_str::<Role>("\"System\"").unwrap(),
            Role::System
        );
        assert_eq!(
            serde_json::from_str::<Role>("\"TOOL\"").unwrap(),
            Role::Tool
        );
        assert_eq!(
            serde_json::from_str::<Role>("\"Tool\"").unwrap(),
            Role::Tool
        );

        // Fallback for unknown role strings
        assert_eq!(
            serde_json::from_str::<Role>("\"developer\"").unwrap(),
            Role::System
        );
        assert_eq!(
            serde_json::from_str::<Role>("\"unknown_role\"").unwrap(),
            Role::System
        );

        // FromStr trait verification
        assert_eq!("user".parse::<Role>().unwrap(), Role::User);
        assert_eq!("ASSISTANT".parse::<Role>().unwrap(), Role::Assistant);
        assert_eq!("unknown".parse::<Role>().unwrap(), Role::System);
    }

    #[test]
    fn test_node_id_12_hex_format() {
        let id = SessionTree::generate_node_id();
        assert_eq!(id.len(), 12);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(id, id.to_lowercase());
    }

    #[test]
    fn test_tree_append_and_history() {
        let mut tree = SessionTree::new();
        let root_id = tree.root_id.clone();
        assert_eq!(tree.active_node_id, root_id);
        assert_eq!(tree.node_count(), 1);

        let user_node_id = tree.append_child(Role::User, "Hello".to_string());
        let assistant_node_id = tree.append_child(Role::Assistant, "Hi there!".to_string());

        let history = tree.get_active_branch_history();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].id, root_id);
        assert_eq!(history[1].id, user_node_id);
        assert_eq!(history[2].id, assistant_node_id);
        assert_eq!(tree.node_count(), 3);
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
    fn test_fork_from_helpers() {
        let mut tree = SessionTree::new();
        let u1 = tree.append_child(Role::User, "Initial question".to_string());
        let _a1 = tree.append_child(Role::Assistant, "First response".to_string());

        // Test fork_from
        assert!(tree.fork_from(&u1));
        assert_eq!(tree.active_node_id, u1);

        // Test fork_from_with_message
        let fork_id = tree
            .fork_from_with_message(&u1, Role::User, "Alternative question".to_string())
            .unwrap();
        assert_eq!(tree.active_node_id, fork_id);

        let history = tree.get_active_branch_history();
        assert_eq!(history.len(), 3);
        assert_eq!(history[1].id, u1);
        assert_eq!(history[2].id, fork_id);

        // Fork from invalid node returns None
        assert!(
            tree.fork_from_with_message("non_existent", Role::User, "msg".to_string())
                .is_none()
        );
    }

    #[test]
    fn test_rewind_invalid_node() {
        let mut tree = SessionTree::new();
        assert!(!tree.rewind_to("non_existent_id"));
        assert!(!tree.fork_from("non_existent_id"));
    }

    #[test]
    fn test_load_from_jsonl() {
        let tmp = tempfile::tempdir().unwrap();
        let session_file = tmp.path().join("test_session.jsonl");

        let mut tree = SessionTree::new_with_disk_path(
            Some(session_file.clone()),
            ".",
            "test-session-123".to_string(),
        );
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
    fn test_load_from_jsonl_without_header_recovery() {
        let tmp = tempfile::tempdir().unwrap();
        let session_file = tmp.path().join("no_header_session.jsonl");

        let raw_jsonl = r#"{"type":"message","id":"msg1","timestamp":"2026-08-16T00:00:00Z","role":"system","content":"Init"}
{"type":"message","id":"msg2","parentId":"msg1","timestamp":"2026-08-16T00:00:01Z","role":"user","content":"Hi"}
{"type":"message","id":"msg3","parentId":"msg2","timestamp":"2026-08-16T00:00:02Z","role":"assistant","content":"Hello"}
"#;
        fs::write(&session_file, raw_jsonl).unwrap();

        let loaded = SessionTree::load_from_jsonl(&session_file).unwrap();
        assert_eq!(loaded.session_id, "no_header_session");
        assert_eq!(loaded.root_id, "msg1");
        assert_eq!(loaded.active_node_id, "msg3");
        assert_eq!(loaded.nodes.len(), 3);
        assert_eq!(loaded.nodes["msg1"].children_ids, vec!["msg2"]);
        assert_eq!(loaded.nodes["msg2"].children_ids, vec!["msg3"]);
    }

    #[test]
    fn test_empty_jsonl_error() {
        let tmp = tempfile::tempdir().unwrap();
        let session_file = tmp.path().join("empty.jsonl");
        fs::write(&session_file, "").unwrap();

        let result = SessionTree::load_from_jsonl(&session_file);
        assert!(result.is_err());
    }

    #[test]
    fn test_circular_reference_safeguard() {
        let mut tree = SessionTree::new();
        let root = tree.root_id.clone();
        let _n1 = tree.append_child(Role::User, "Step 1".to_string());
        let n2 = tree.append_child(Role::Assistant, "Step 2".to_string());

        // Create artificial 3-node cycle: root -> n1 -> n2 -> root
        if let Some(r_node) = tree.nodes.get_mut(&root) {
            r_node.parent_id = Some(n2.clone());
        }

        let history = tree.get_branch_history(&n2);
        // History terminates without infinite loop
        assert!(history.len() <= 3);
    }

    #[test]
    fn test_malformed_and_truncated_jsonl_recovery() {
        let tmp = tempfile::tempdir().unwrap();
        let session_file = tmp.path().join("corrupted.jsonl");
        fs::write(
            &session_file,
            r#"{"type":"session","version":3,"id":"corrupt-1","timestamp":"2026-08-15T00:00:00Z","cwd":"."}
{"type":"unknown","id":""}
GARBAGE_LINE_NOT_JSON
{"type":"message","id":"msg1","timestamp":"2026-08-15T00:00:01Z","role":"user","content":"Valid 1"}
{"type":"message","id":"msg2","parentId":"msg1","timestamp":"2026-08-15T00:00:02Z","role":"assistant","content":"Valid 2"}
{"type":"message","id":"incomplete_msg","parentId":"msg2","content":"Trunca
"#,
        )
        .unwrap();

        let loaded = SessionTree::load_from_jsonl(&session_file).unwrap();
        let history = loaded.get_active_branch_history();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].content, "Valid 1");
        assert_eq!(history[1].content, "Valid 2");
        assert_eq!(loaded.active_node_id, "msg2");
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

        // Test find_lca helper
        assert_eq!(tree.find_lca(&u2_a, &a2_b), Some(u1.clone()));

        // Test identical nodes (no divergence)
        let same_diff = tree.diff_branches(&u2_a, &u2_a);
        assert_eq!(same_diff.lca_node_id, Some(u2_a.clone()));
        assert!(same_diff.branch_a_divergent.is_empty());
        assert!(same_diff.branch_b_divergent.is_empty());

        // Test diff with root
        let root_diff = tree.diff_branches(&root_id, &u1);
        assert_eq!(root_diff.lca_node_id, Some(root_id));
        assert!(root_diff.branch_a_divergent.is_empty());
        assert_eq!(root_diff.branch_b_divergent.len(), 1);
        assert_eq!(root_diff.branch_b_divergent[0].id, u1);

        // Test ancestor relation
        assert!(tree.is_ancestor_of(&u1, &a2_b));
        assert!(!tree.is_ancestor_of(&u2_a, &a2_b));
    }

    #[test]
    fn test_deep_branching_dag_and_inspection_methods() {
        let mut tree = SessionTree::new();
        let root = tree.root_id.clone();

        // Main trunk: root -> u1 -> a1
        let u1 = tree.append_child(Role::User, "U1".to_string());
        assert!(tree.contains_node(&u1));
        let a1 = tree.append_child(Role::Assistant, "A1".to_string());

        // Sub-branch 1: a1 -> u2 -> a2 -> u3 -> a3
        let u2 = tree.append_child(Role::User, "U2".to_string());
        let a2 = tree.append_child(Role::Assistant, "A2".to_string());
        let u3 = tree.append_child(Role::User, "U3".to_string());
        let a3 = tree.append_child(Role::Assistant, "A3".to_string());

        // Sub-branch 2: a1 -> u4 -> a4
        tree.rewind_to(&a1);
        let u4 = tree.append_child(Role::User, "U4".to_string());
        let a4 = tree.append_child(Role::Assistant, "A4".to_string());

        // Sub-branch 3: a2 -> u5
        tree.rewind_to(&a2);
        let u5 = tree.append_child(Role::User, "U5".to_string());

        // Inspection methods
        assert_eq!(tree.node_count(), 10);
        assert!(tree.contains_node(&u5));
        assert!(!tree.contains_node("missing"));

        // Children of a1 are u2 and u4
        let children_a1 = tree.get_children(&a1);
        assert_eq!(children_a1.len(), 2);
        assert_eq!(children_a1[0].id, u2);
        assert_eq!(children_a1[1].id, u4);

        // Children of a2 are u3 and u5
        let children_a2 = tree.get_children(&a2);
        assert_eq!(children_a2.len(), 2);
        assert_eq!(children_a2[0].id, u3);
        assert_eq!(children_a2[1].id, u5);

        // Leaves are a3, a4, u5
        let leaf_ids: HashSet<String> = tree
            .get_leaf_nodes()
            .into_iter()
            .map(|n| n.id.clone())
            .collect();
        assert_eq!(leaf_ids.len(), 3);
        assert!(leaf_ids.contains(&a3));
        assert!(leaf_ids.contains(&a4));
        assert!(leaf_ids.contains(&u5));
        assert_eq!(tree.branch_count(), 3);

        // Tree depth: root -> u1 -> a1 -> u2 -> a2 -> u3 -> a3 = 7 nodes
        assert_eq!(tree.tree_depth(), 7);

        // LCA checks across deep branches
        assert_eq!(tree.find_lca(&a3, &u5), Some(a2));
        assert_eq!(tree.find_lca(&a3, &a4), Some(a1));
        assert_eq!(tree.find_lca(&a4, &root), Some(root));
    }

    #[test]
    fn test_lowercase_role_jsonl_serialization_and_reconstruction() {
        let temp_dir = tempfile::tempdir().unwrap();
        let session_file = temp_dir.path().join("test_session.jsonl");

        let raw_jsonl = r#"{"type":"session","version":3,"id":"sess-123","timestamp":"2026-08-16T00:00:00Z","cwd":"/test"}
{"type":"node","id":"node-1","role":"system","content":"System init","timestamp":"2026-08-16T00:00:01Z"}
{"type":"node","id":"node-2","parentId":"node-1","role":"USER","content":"Hello","timestamp":"2026-08-16T00:00:02Z"}
{"type":"node","id":"node-3","parent_id":"node-2","role":"Assistant","content":"Hi there","timestamp":"2026-08-16T00:00:03Z"}
{"type":"node","id":"node-4","parentId":"node-3","role":"TOOL","content":"Tool output","toolName":"read","timestamp":"2026-08-16T00:00:04Z"}
"#;
        std::fs::write(&session_file, raw_jsonl).unwrap();

        let loaded = SessionTree::load_from_jsonl(&session_file).unwrap();
        assert_eq!(loaded.nodes.len(), 4);
        assert_eq!(loaded.nodes["node-1"].role, Role::System);
        assert_eq!(loaded.nodes["node-2"].role, Role::User);
        assert_eq!(loaded.nodes["node-3"].role, Role::Assistant);
        assert_eq!(loaded.nodes["node-4"].role, Role::Tool);

        // Verify children_ids were properly reconstructed
        assert_eq!(loaded.nodes["node-1"].children_ids, vec!["node-2"]);
        assert_eq!(loaded.nodes["node-2"].children_ids, vec!["node-3"]);
        assert_eq!(loaded.nodes["node-3"].children_ids, vec!["node-4"]);
        assert_eq!(loaded.nodes["node-4"].tool_name.as_deref(), Some("read"));
    }
}
