use crate::AgentLoop;
use anyhow::{Result, anyhow};
use chrono::{Duration, Utc};
use dirs::home_dir;
use pi_providers::ModelConfig;
use pi_session::SessionTree;
use pi_tools::{InvokeSubagentArgs, ManageSubagentsArgs, SubagentToolHandler};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::future::Future;
use std::io::Write;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentConfig {
    pub name: String,
    #[serde(default)]
    pub model_override: Option<String>,
    #[serde(default)]
    pub system_prompt_override: Option<String>,
    #[serde(default)]
    pub allowed_tools: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SubagentStatus {
    Running,
    Idle,
    Finished(String),
    Errored(String),
}

impl std::fmt::Display for SubagentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running => write!(f, "Running"),
            Self::Idle => write!(f, "Idle"),
            Self::Finished(out) => write!(f, "Finished({})", out),
            Self::Errored(err) => write!(f, "Errored({})", err),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentSummary {
    pub id: String,
    pub name: String,
    pub task: String,
    pub status: String,
    pub created_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentMessage {
    pub id: String,
    pub from: String,
    pub to: String,
    pub body: String,
    pub timestamp: String,
    pub delivered: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentSubagentRecord {
    pub name: String,
    pub config: SubagentConfig,
    pub session_path: Option<PathBuf>,
    pub status: SubagentStatus,
    pub created_at: String,
    pub finished_at: Option<String>,
    pub updated_at: String,
}

#[derive(Clone)]
pub struct SubagentInstance {
    pub id: Uuid,
    pub name: String,
    pub task_description: String,
    pub agent_loop: AgentLoop,
    pub status: SubagentStatus,
    pub cancellation_token: CancellationToken,
    pub created_at: chrono::DateTime<Utc>,
    pub finished_at: Option<chrono::DateTime<Utc>>,
    pub persistent_name: Option<String>,
    pub messages: Arc<RwLock<VecDeque<SubagentMessage>>>,
}

pub struct SubagentRunner;

impl SubagentRunner {
    pub async fn run_task(
        mut agent_loop: AgentLoop,
        task: String,
        cancellation_token: CancellationToken,
    ) -> Result<String> {
        tokio::select! {
            _ = cancellation_token.cancelled() => {
                Err(anyhow!("Subagent task was cancelled"))
            }
            res = agent_loop.run_turn(&task, |_| {}) => {
                res
            }
        }
    }
}

#[derive(Clone)]
pub struct SubagentManager {
    pub default_model_config: ModelConfig,
    instances: Arc<RwLock<HashMap<String, SubagentInstance>>>,
    pub persist_dir: Option<PathBuf>,
}

impl SubagentManager {
    pub fn new(default_model_config: ModelConfig) -> Self {
        Self {
            default_model_config,
            instances: Arc::new(RwLock::new(HashMap::new())),
            persist_dir: None,
        }
    }

    pub fn new_with_empty_model() -> Self {
        let dummy_model = ModelConfig::resolve("mock/mock-model");
        Self::new(dummy_model)
    }

    fn tau_state_dir() -> PathBuf {
        let home = home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join(".tau").join("subagents")
    }

    fn sanitize_name(name: &str) -> String {
        name.chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '-'
                }
            })
            .collect()
    }

    fn build_agent_loop(&self, config: SubagentConfig) -> Result<AgentLoop> {
        let mut child_model = self.default_model_config.clone();
        if let Some(ref m) = config.model_override {
            child_model.model_id = m.clone();
        }
        let mut child_agent_loop = AgentLoop::new(child_model);
        if let Some(ref sp) = config.system_prompt_override {
            child_agent_loop.system_engine.base_prompt = sp.clone();
        }
        if let Some(ref tools) = config.allowed_tools {
            child_agent_loop.allowed_tools = Some(tools.clone());
        }
        Ok(child_agent_loop)
    }

    async fn load_registry_entry(
        &self,
        registry_path: &PathBuf,
        name: &str,
    ) -> Result<Option<PersistentSubagentRecord>> {
        if !registry_path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(registry_path)?;
        let mut latest: Option<PersistentSubagentRecord> = None;
        for line in content.lines() {
            if let Ok(record) = serde_json::from_str::<PersistentSubagentRecord>(line)
                && record.name == name
            {
                latest = Some(record);
            }
        }
        Ok(latest)
    }

    async fn append_registry_entry(
        &self,
        registry_path: &PathBuf,
        record: &PersistentSubagentRecord,
    ) -> Result<()> {
        if let Ok(json) = serde_json::to_string(record) {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(registry_path)?
                .write_all(format!("{}\n", json).as_bytes())?;
        }
        Ok(())
    }

    async fn overwrite_registry(
        registry_path: &PathBuf,
        records: &[PersistentSubagentRecord],
    ) -> Result<()> {
        let mut file = std::fs::File::create(registry_path)?;
        for record in records {
            if let Ok(json) = serde_json::to_string(record) {
                writeln!(file, "{}", json)?;
            }
        }
        Ok(())
    }

    pub async fn spawn(&self, config: SubagentConfig, task: &str) -> Result<String> {
        let subagent_id = Uuid::new_v4();
        let id_str = subagent_id.to_string();

        let mut child_model = self.default_model_config.clone();
        if let Some(ref m) = config.model_override {
            child_model.model_id = m.clone();
        }

        let mut child_agent_loop = AgentLoop::new(child_model);
        if let Some(ref sp) = config.system_prompt_override {
            child_agent_loop.system_engine.base_prompt = sp.clone();
        }
        if let Some(ref tools) = config.allowed_tools {
            child_agent_loop.allowed_tools = Some(tools.clone());
        }

        let cancellation_token = CancellationToken::new();

        let instance = SubagentInstance {
            id: subagent_id,
            name: config.name.clone(),
            task_description: task.to_string(),
            agent_loop: child_agent_loop.clone(),
            status: SubagentStatus::Running,
            cancellation_token: cancellation_token.clone(),
            created_at: Utc::now(),
            finished_at: None,
            persistent_name: None,
            messages: Arc::new(RwLock::new(VecDeque::new())),
        };

        {
            let mut guard = self.instances.write().await;
            guard.insert(id_str.clone(), instance);
        }

        let instances_clone = self.instances.clone();
        let id_clone = id_str.clone();
        let task_owned = task.to_string();

        tokio::spawn(async move {
            let result =
                SubagentRunner::run_task(child_agent_loop, task_owned, cancellation_token).await;

            let mut guard = instances_clone.write().await;
            if let Some(inst) = guard.get_mut(&id_clone) {
                inst.finished_at = Some(Utc::now());
                match result {
                    Ok(output) => {
                        inst.status = SubagentStatus::Finished(output);
                    }
                    Err(err) => {
                        inst.status = SubagentStatus::Errored(err.to_string());
                    }
                }
            }
        });

        Ok(id_str)
    }

    /// Spawns or re-addressing a persistent named subagent. If a finished persistent
    /// subagent with the same name exists, its prior session context is loaded for a
    /// follow-up turn. Sibling-to-sibling and child-to-parent messaging are supported;
    /// there are no additional scope restrictions beyond name-based addressing.
    pub async fn spawn_persistent(&self, config: SubagentConfig, task: &str) -> Result<String> {
        let name = config.name.clone();
        let persist_dir = self.persist_dir.clone().unwrap_or_else(Self::tau_state_dir);
        let registry_path = persist_dir.join("registry.jsonl");
        let inbox_dir = persist_dir.join(Self::sanitize_name(&name));
        let _ = fs::create_dir_all(&inbox_dir);

        let existing = self.load_registry_entry(&registry_path, &name).await?;

        let mut agent_loop = self.build_agent_loop(config.clone())?;
        let session_path = agent_loop.session_tree.disk_path.clone();

        if let Some(record) = existing {
            match record.status {
                SubagentStatus::Finished(_) | SubagentStatus::Errored(_) => {
                    if let Some(ref p) = record.session_path
                        && p.exists()
                    {
                        agent_loop = self.build_agent_loop(config.clone())?;
                        agent_loop.session_tree = SessionTree::load_from_jsonl(p)?;
                    }
                }
                SubagentStatus::Running => {
                    let stale_ids: Vec<String> = {
                        let guard = self.instances.read().await;
                        guard
                            .values()
                            .filter(|inst| inst.persistent_name.as_deref() == Some(&name))
                            .map(|inst| inst.id.to_string())
                            .collect()
                    };
                    // Cancel any prior turn(s) for this persistent name; a
                    // repeated spawn_persistent is an intentional continuation.
                    let mut guard = self.instances.write().await;
                    for id in &stale_ids {
                        if let Some(inst) = guard.get_mut(id) {
                            inst.cancellation_token.cancel();
                            inst.status =
                                SubagentStatus::Errored("superseded by newer spawn".to_string());
                            inst.finished_at = Some(Utc::now());
                        }
                    }
                    // Registry line still says Running from the old turn; the
                    // new record appended below supersedes it (load takes latest).
                }
                _ => {}
            }
        }

        let subagent_id = Uuid::new_v4();
        let id_str = subagent_id.to_string();
        let cancellation_token = CancellationToken::new();
        let now = Utc::now();

        let instance = SubagentInstance {
            id: subagent_id,
            name: name.clone(),
            task_description: task.to_string(),
            agent_loop,
            status: SubagentStatus::Running,
            cancellation_token: cancellation_token.clone(),
            created_at: now,
            finished_at: None,
            persistent_name: Some(name.clone()),
            messages: Arc::new(RwLock::new(VecDeque::new())),
        };

        {
            let mut guard = self.instances.write().await;
            guard.insert(id_str.clone(), instance.clone());
        }

        let record = PersistentSubagentRecord {
            name: name.clone(),
            config: config.clone(),
            session_path: session_path.clone(),
            status: SubagentStatus::Running,
            created_at: now.to_rfc3339(),
            finished_at: None,
            updated_at: now.to_rfc3339(),
        };
        self.append_registry_entry(&registry_path, &record).await?;

        let instances_clone = self.instances.clone();
        let id_clone = id_str.clone();
        let task_owned = task.to_string();
        let config_clone = config.clone();
        let registry_path_clone = registry_path.clone();
        let persistent_name_clone = name.clone();
        let manager_self = self.clone();

        tokio::spawn(async move {
            let result =
                SubagentRunner::run_task(instance.agent_loop, task_owned, cancellation_token).await;

            let mut guard = instances_clone.write().await;
            if let Some(inst) = guard.get_mut(&id_clone) {
                inst.finished_at = Some(Utc::now());
                match result {
                    Ok(output) => inst.status = SubagentStatus::Finished(output),
                    Err(err) => inst.status = SubagentStatus::Errored(err.to_string()),
                }

                let updated = PersistentSubagentRecord {
                    name: persistent_name_clone,
                    config: config_clone,
                    session_path: inst.agent_loop.session_tree.disk_path.clone(),
                    status: inst.status.clone(),
                    created_at: inst.created_at.to_rfc3339(),
                    finished_at: inst.finished_at.map(|t| t.to_rfc3339()),
                    updated_at: Utc::now().to_rfc3339(),
                };
                let _ = manager_self
                    .append_registry_entry(&registry_path_clone, &updated)
                    .await;
            }
        });

        Ok(name)
    }

    /// Sends an A2A message from one agent to another. Messages are append-only on disk
    /// and queued in memory if the recipient is currently loaded. Delivery occurs at the
    /// recipient's next turn boundary.
    pub async fn send_message(&self, from: &str, to: &str, body: &str) -> Result<()> {
        let message = SubagentMessage {
            id: Uuid::new_v4().to_string(),
            from: from.to_string(),
            to: to.to_string(),
            body: body.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            delivered: false,
        };

        let persist_dir = self.persist_dir.clone().unwrap_or_else(Self::tau_state_dir);
        let inbox_path = persist_dir
            .join(Self::sanitize_name(to))
            .join("inbox.jsonl");

        if let Ok(json) = serde_json::to_string(&message) {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(inbox_path)?
                .write_all(format!("{}\n", json).as_bytes())?;
        }

        for inst in self.instances.read().await.values() {
            if inst.persistent_name.as_deref() == Some(to) || inst.name == to {
                inst.messages.write().await.push_back(message.clone());
                break;
            }
        }

        Ok(())
    }

    /// Drains the inbox for a named subagent. Returns all pending messages and clears
    /// the in-memory queue. If the subagent is not currently loaded, reads from disk
    /// and truncates the inbox file.
    pub async fn drain_inbox(&self, name: &str) -> Result<Vec<SubagentMessage>> {
        let mut messages = Vec::new();

        for inst in self.instances.read().await.values() {
            if inst.persistent_name.as_deref() == Some(name) || inst.name == name {
                let mut queue = inst.messages.write().await;
                messages.extend(queue.drain(..));
                break;
            }
        }

        // Always consume the on-disk inbox too; it may hold messages written
        // by other processes or before this instance was registered. Clearing
        // it here prevents redelivery on the next drain.
        let persist_dir = self.persist_dir.clone().unwrap_or_else(Self::tau_state_dir);
        let inbox_path = persist_dir
            .join(Self::sanitize_name(name))
            .join("inbox.jsonl");
        if inbox_path.exists() {
            let content = std::fs::read_to_string(&inbox_path)?;
            for line in content.lines() {
                if let Ok(msg) = serde_json::from_str::<SubagentMessage>(line)
                    && !messages.iter().any(|m| m.id == msg.id)
                {
                    messages.push(msg);
                }
            }
            let _ = std::fs::write(&inbox_path, "");
        }

        Ok(messages)
    }

    /// Lists all persistent subagent records currently stored on disk.
    pub async fn list_persistent(&self) -> Result<Vec<PersistentSubagentRecord>> {
        let persist_dir = self.persist_dir.clone().unwrap_or_else(Self::tau_state_dir);
        let registry_path = persist_dir.join("registry.jsonl");

        if !registry_path.exists() {
            return Ok(Vec::new());
        }

        let mut records = Vec::new();
        let content = std::fs::read_to_string(&registry_path)?;
        for line in content.lines() {
            if let Ok(record) = serde_json::from_str::<PersistentSubagentRecord>(line) {
                records.push(record);
            }
        }
        Ok(records)
    }

    /// Recovers persistent subagent registries from disk into the manager's in-memory
    /// view. Returns the number of non-running records recovered.
    pub async fn recover(&self) -> Result<usize> {
        let records = self.list_persistent().await?;
        let persist_dir = self.persist_dir.clone().unwrap_or_else(Self::tau_state_dir);
        let registry_path = persist_dir.join("registry.jsonl");
        let mut count = 0usize;
        for record in records {
            match record.status {
                SubagentStatus::Running => {
                    // A Running record with no live in-memory instance is a
                    // leftover from a dead process: mark it Errored on disk.
                    let live = self
                        .instances
                        .read()
                        .await
                        .values()
                        .any(|inst| inst.persistent_name.as_deref() == Some(&record.name));
                    if !live {
                        let mut updated = record.clone();
                        updated.status =
                            SubagentStatus::Errored("recovered from interrupted run".to_string());
                        updated.updated_at = Utc::now().to_rfc3339();
                        self.append_registry_entry(&registry_path, &updated).await?;
                        count += 1;
                    }
                }
                _ => count += 1,
            }
        }
        Ok(count)
    }

    /// Prunes persistent subagent records that have been idle longer than `max_idle`.
    /// Removes registry entries and their inbox directories from disk.
    pub async fn prune(&self, max_idle: Duration) -> Result<usize> {
        let persist_dir = self.persist_dir.clone().unwrap_or_else(Self::tau_state_dir);
        let cutoff = Utc::now() - max_idle;

        let records = self.list_persistent().await?;
        let mut pruned = 0usize;
        let mut kept = Vec::new();

        for record in records {
            if let Some(finished_at) = record.finished_at.as_ref()
                && let Ok(time) = chrono::DateTime::parse_from_rfc3339(finished_at)
            {
                let time = time.with_timezone(&Utc);
                if time < cutoff {
                    let inbox_dir = persist_dir.join(Self::sanitize_name(&record.name));
                    let _ = std::fs::remove_dir_all(&inbox_dir);
                    pruned += 1;
                    continue;
                }
            }
            kept.push(record);
        }

        let kept_names: std::collections::HashSet<_> =
            kept.iter().map(|r| r.name.clone()).collect();
        Self::overwrite_registry(&persist_dir.join("registry.jsonl"), &kept).await?;

        let mut guard = self.instances.write().await;
        guard.retain(|_, inst| {
            inst.persistent_name
                .as_ref()
                .map(|n| !kept_names.contains(n.as_str()))
                .unwrap_or(true)
        });

        Ok(pruned)
    }

    pub async fn invoke_and_wait(&self, config: SubagentConfig, task: &str) -> Result<String> {
        let subagent_id = Uuid::new_v4();
        let id_str = subagent_id.to_string();

        let mut child_model = self.default_model_config.clone();
        if let Some(ref m) = config.model_override {
            child_model.model_id = m.clone();
        }

        let mut child_agent_loop = AgentLoop::new(child_model);
        if let Some(ref sp) = config.system_prompt_override {
            child_agent_loop.system_engine.base_prompt = sp.clone();
        }
        if let Some(ref tools) = config.allowed_tools {
            child_agent_loop.allowed_tools = Some(tools.clone());
        }

        let cancellation_token = CancellationToken::new();

        let instance = SubagentInstance {
            id: subagent_id,
            name: config.name.clone(),
            task_description: task.to_string(),
            agent_loop: child_agent_loop.clone(),
            status: SubagentStatus::Running,
            cancellation_token: cancellation_token.clone(),
            created_at: Utc::now(),
            finished_at: None,
            persistent_name: None,
            messages: Arc::new(RwLock::new(VecDeque::new())),
        };

        {
            let mut guard = self.instances.write().await;
            guard.insert(id_str.clone(), instance);
        }

        let task_owned = task.to_string();
        let result =
            SubagentRunner::run_task(child_agent_loop, task_owned, cancellation_token).await;

        let mut guard = self.instances.write().await;
        if let Some(inst) = guard.get_mut(&id_str) {
            inst.finished_at = Some(Utc::now());
            match &result {
                Ok(output) => {
                    inst.status = SubagentStatus::Finished(output.clone());
                }
                Err(err) => {
                    inst.status = SubagentStatus::Errored(err.to_string());
                }
            }
        }

        result
    }

    pub async fn list(&self) -> Vec<SubagentSummary> {
        let guard = self.instances.read().await;
        guard
            .values()
            .map(|inst| SubagentSummary {
                id: inst.id.to_string(),
                name: inst.name.clone(),
                task: inst.task_description.clone(),
                status: inst.status.to_string(),
                created_at: inst.created_at.to_rfc3339(),
                finished_at: inst.finished_at.map(|t| t.to_rfc3339()),
            })
            .collect()
    }

    pub async fn get_status(&self, id: &str) -> Result<SubagentStatus> {
        let guard = self.instances.read().await;
        let inst = guard
            .get(id)
            .ok_or_else(|| anyhow!("Subagent '{}' not found", id))?;
        Ok(inst.status.clone())
    }

    pub async fn kill(&self, id: &str) -> Result<bool> {
        let mut guard = self.instances.write().await;
        if let Some(inst) = guard.get_mut(id) {
            inst.cancellation_token.cancel();
            inst.status = SubagentStatus::Errored("Subagent cancelled by user".to_string());
            inst.finished_at = Some(Utc::now());
            Ok(true)
        } else {
            Err(anyhow!("Subagent '{}' not found", id))
        }
    }

    pub async fn kill_all(&self) -> usize {
        let mut guard = self.instances.write().await;
        let mut count = 0;
        let now = Utc::now();
        for inst in guard.values_mut() {
            if inst.status == SubagentStatus::Running {
                inst.cancellation_token.cancel();
                inst.status = SubagentStatus::Errored("Subagent cancelled by user".to_string());
                inst.finished_at = Some(now);
                count += 1;
            }
        }
        count
    }

    pub fn create_tool_handler(self: &Arc<Self>) -> Arc<dyn SubagentToolHandler> {
        Arc::new(SubagentToolHandlerBridge {
            manager: self.clone(),
        })
    }

    pub fn init_global_handler(self: &Arc<Self>) {
        let handler = self.create_tool_handler();
        pi_tools::register_subagent_handler(handler);
    }
}

struct SubagentToolHandlerBridge {
    manager: Arc<SubagentManager>,
}

impl SubagentToolHandler for SubagentToolHandlerBridge {
    fn invoke<'a>(
        &'a self,
        args: &'a InvokeSubagentArgs,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            let config = SubagentConfig {
                name: args.name.clone(),
                model_override: args.model.clone(),
                system_prompt_override: None,
                allowed_tools: args.tools.clone(),
            };

            // If task is short or synchronous delegation requested, we can invoke and wait or spawn
            let id = self.manager.spawn(config, &args.task).await?;
            Ok(format!(
                "Spawned subagent [{}] (ID: {}) for task: {}",
                args.name, id, args.task
            ))
        })
    }

    fn manage<'a>(
        &'a self,
        args: &'a ManageSubagentsArgs,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            match args.action.as_str() {
                "list" => {
                    let list = self.manager.list().await;
                    serde_json::to_string_pretty(&list)
                        .map_err(|e| anyhow!("Failed to serialize subagent list: {}", e))
                }
                "status" => {
                    let id = args
                        .id
                        .as_deref()
                        .ok_or_else(|| anyhow!("Missing 'id' for status action"))?;
                    let status = self.manager.get_status(id).await?;
                    Ok(format!("Subagent {} status: {}", id, status))
                }
                "kill" => {
                    let id = args
                        .id
                        .as_deref()
                        .ok_or_else(|| anyhow!("Missing 'id' for kill action"))?;
                    self.manager.kill(id).await?;
                    Ok(format!("Subagent {} successfully cancelled", id))
                }
                other => Err(anyhow!("Unknown manage_subagents action: {}", other)),
            }
        })
    }
}

impl SubagentManager {
    /// Creates a manager pre-configured with a persistence directory.
    pub fn with_persist_dir(mut self, dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&dir);
        self.persist_dir = Some(dir);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_subagent_lifecycle_and_cancellation() {
        let manager = Arc::new(SubagentManager::new_with_empty_model());
        let config = SubagentConfig {
            name: "TestWorker".to_string(),
            model_override: None,
            system_prompt_override: Some("You are a test worker".to_string()),
            allowed_tools: None,
        };

        let id = manager
            .spawn(config, "Compute fibonacci sequence")
            .await
            .unwrap();
        assert!(!id.is_empty());

        let list = manager.list().await;
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "TestWorker");

        // Test cancellation
        let killed = manager.kill(&id).await.unwrap();
        assert!(killed);

        let status = manager.get_status(&id).await.unwrap();
        assert!(matches!(status, SubagentStatus::Errored(_)));
    }

    #[tokio::test]
    async fn test_subagent_tool_handler_bridge() {
        let manager = Arc::new(SubagentManager::new_with_empty_model());
        let handler = manager.create_tool_handler();

        let invoke_args = InvokeSubagentArgs {
            name: "Reviewer".to_string(),
            task: "Review PR #42".to_string(),
            model: None,
            tools: Some(vec!["read".to_string(), "grep".to_string()]),
        };

        let invoke_res = handler.invoke(&invoke_args).await.unwrap();
        assert!(invoke_res.contains("Spawned subagent [Reviewer]"));

        let list_args = ManageSubagentsArgs {
            action: "list".to_string(),
            id: None,
        };
        let list_res = handler.manage(&list_args).await.unwrap();
        assert!(list_res.contains("Reviewer"));
        assert!(list_res.contains("Review PR #42"));
    }

    #[tokio::test]
    async fn test_subagent_kill_all() {
        let manager = Arc::new(SubagentManager::new_with_empty_model());
        for i in 1..=3 {
            let config = SubagentConfig {
                name: format!("Worker-{}", i),
                model_override: None,
                system_prompt_override: None,
                allowed_tools: Some(vec!["read".to_string()]),
            };
            let _ = manager.spawn(config, "Task").await.unwrap();
        }

        let list_before = manager.list().await;
        assert_eq!(list_before.len(), 3);

        let killed_count = manager.kill_all().await;
        assert_eq!(killed_count, 3);

        for summary in manager.list().await {
            assert!(summary.status.contains("Errored") || summary.status.contains("cancelled"));
        }
    }

    #[tokio::test]
    async fn test_subagent_unknown_action_and_not_found() {
        let manager = Arc::new(SubagentManager::new_with_empty_model());
        let handler = manager.create_tool_handler();

        let unknown_action = ManageSubagentsArgs {
            action: "explode".to_string(),
            id: None,
        };
        assert!(handler.manage(&unknown_action).await.is_err());

        let missing_id = ManageSubagentsArgs {
            action: "status".to_string(),
            id: None,
        };
        assert!(handler.manage(&missing_id).await.is_err());

        let non_existent = ManageSubagentsArgs {
            action: "status".to_string(),
            id: Some("non-existent-id".to_string()),
        };
        assert!(handler.manage(&non_existent).await.is_err());

        let kill_non_existent = manager.kill("non-existent-id").await;
        assert!(kill_non_existent.is_err());
    }

    #[tokio::test]
    async fn test_persistent_subagent_send_then_drain_roundtrip() {
        let tmp = tempdir().unwrap();
        let manager = Arc::new(
            SubagentManager::new_with_empty_model().with_persist_dir(tmp.path().to_path_buf()),
        );

        let config = SubagentConfig {
            name: "Follower".to_string(),
            model_override: None,
            system_prompt_override: None,
            allowed_tools: None,
        };

        let _ = manager
            .spawn_persistent(config.clone(), "Initial task")
            .await
            .unwrap();

        manager
            .send_message("Leader", "Follower", "Hello from leader")
            .await
            .unwrap();

        let messages = manager.drain_inbox("Follower").await.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].from, "Leader");
        assert_eq!(messages[0].body, "Hello from leader");

        let empty = manager.drain_inbox("Follower").await.unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn test_persistent_subagent_survives_drop_and_recover() {
        let tmp = tempdir().unwrap();
        let persist_path = tmp.path().to_path_buf();

        {
            let manager =
                SubagentManager::new_with_empty_model().with_persist_dir(persist_path.clone());
            let config = SubagentConfig {
                name: "Survivor".to_string(),
                model_override: None,
                system_prompt_override: None,
                allowed_tools: None,
            };
            manager
                .spawn_persistent(config, "First turn")
                .await
                .unwrap();
        }

        let manager = SubagentManager::new_with_empty_model().with_persist_dir(persist_path);
        let recovered = manager.recover().await.unwrap();
        assert!(recovered >= 1);

        let persistent = manager.list_persistent().await.unwrap();
        assert!(persistent.iter().any(|r| r.name == "Survivor"));
    }

    #[tokio::test]
    async fn test_persistent_follow_up_preserves_name_and_config() {
        let tmp = tempdir().unwrap();
        let manager = Arc::new(
            SubagentManager::new_with_empty_model().with_persist_dir(tmp.path().to_path_buf()),
        );

        let config = SubagentConfig {
            name: "Resume".to_string(),
            model_override: Some("mock/override-model".to_string()),
            system_prompt_override: Some("System prompt".to_string()),
            allowed_tools: Some(vec!["read".to_string()]),
        };

        manager
            .spawn_persistent(config.clone(), "First task")
            .await
            .unwrap();

        manager
            .send_message("self", "Resume", "follow-up context")
            .await
            .unwrap();

        let _ = manager
            .spawn_persistent(config.clone(), "Second task")
            .await
            .unwrap();

        let records = manager.list_persistent().await.unwrap();
        let record = records.iter().find(|r| r.name == "Resume").unwrap();
        assert_eq!(record.config.name, "Resume");
        assert_eq!(
            record.config.model_override,
            Some("mock/override-model".to_string())
        );
        assert_eq!(
            record.config.system_prompt_override,
            Some("System prompt".to_string())
        );
    }

    #[tokio::test]
    async fn test_persistent_concurrent_sends_do_not_corrupt_inbox() {
        let tmp = tempdir().unwrap();
        let manager = Arc::new(
            SubagentManager::new_with_empty_model().with_persist_dir(tmp.path().to_path_buf()),
        );

        let config = SubagentConfig {
            name: "Concurrent".to_string(),
            model_override: None,
            system_prompt_override: None,
            allowed_tools: None,
        };

        manager
            .spawn_persistent(config, "Concurrent task")
            .await
            .unwrap();

        let mut handles = Vec::new();
        for i in 0..20 {
            let manager = manager.clone();
            handles.push(tokio::spawn(async move {
                manager
                    .send_message(
                        &format!("sender-{}", i),
                        "Concurrent",
                        &format!("msg-{}", i),
                    )
                    .await
            }));
        }
        for handle in handles {
            handle.await.unwrap().unwrap();
        }

        let messages = manager.drain_inbox("Concurrent").await.unwrap();
        assert_eq!(messages.len(), 20);
        let bodies: Vec<String> = messages.into_iter().map(|m| m.body).collect();
        for i in 0..20 {
            assert!(bodies.contains(&format!("msg-{}", i)));
        }
    }
}
