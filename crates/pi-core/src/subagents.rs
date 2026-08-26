use crate::AgentLoop;
use crate::subagent_persistence::{ModelConfigSnapshot, PersistedSubagent, SubagentPersistence};
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use pi_providers::ModelConfig;
use pi_tools::{InvokeSubagentArgs, ManageSubagentsArgs, SubagentToolHandler};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
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
}

pub struct SubagentRunner;

impl SubagentRunner {
    pub async fn run_task(
        mut agent_loop: AgentLoop,
        task: String,
        cancellation_token: CancellationToken,
    ) -> Result<String> {
        tokio::select! {
            _ = cancellation_token.cancelled() => Err(anyhow!("Subagent task was cancelled")),
            res = agent_loop.run_turn(&task, |_| {}) => res,
        }
    }
}

#[derive(Clone)]
pub struct SubagentManager {
    pub default_model_config: ModelConfig,
    instances: Arc<RwLock<HashMap<String, SubagentInstance>>>,
    persistence: Option<SubagentPersistence>,
}

impl SubagentManager {
    pub fn new(default_model_config: ModelConfig) -> Self {
        Self {
            default_model_config,
            instances: Arc::new(RwLock::new(HashMap::new())),
            persistence: None,
        }
    }

    pub fn new_with_empty_model() -> Self {
        let dummy_model = ModelConfig::resolve("mock/mock-model");
        Self::new(dummy_model)
    }

    pub fn with_persistence(mut self, persistence: SubagentPersistence) -> Self {
        self.persistence = Some(persistence);
        self
    }

    pub fn persistence(&self) -> Option<&SubagentPersistence> {
        self.persistence.as_ref()
    }

    pub async fn load_persisted<P: AsRef<std::path::Path>>(
        default_model_config: ModelConfig,
        data_dir: P,
    ) -> Result<Self> {
        let persistence = SubagentPersistence::new(data_dir)?;
        let manager = Self {
            default_model_config,
            instances: Arc::new(RwLock::new(HashMap::new())),
            persistence: Some(persistence.clone()),
        };

        for persisted in persistence.list().await {
            manager.restore_persisted(persisted).await?;
        }

        Ok(manager)
    }

    async fn restore_persisted(&self, persisted: PersistedSubagent) -> Result<()> {
        let model_cfg = if let Some(ref model_snap) = persisted.model_config {
            ModelConfig {
                provider: model_snap.provider.clone(),
                model_id: model_snap.model_id.clone(),
                api_key: model_snap.api_key.clone().unwrap_or_default(),
                base_url: model_snap.base_url.clone(),
                context_window: model_snap.context_window,
                max_output: model_snap.max_output,
            }
        } else {
            self.default_model_config.clone()
        };

        let Ok(id) = Uuid::parse_str(&persisted.id) else {
            return Ok(());
        };

        let status = match persisted.status.as_str() {
            "Running" => SubagentStatus::Running,
            "Idle" => SubagentStatus::Idle,
            "Finished" => SubagentStatus::Finished(persisted.output.unwrap_or_default()),
            "Errored" => SubagentStatus::Errored(persisted.output.unwrap_or_default()),
            other => SubagentStatus::Errored(format!("Unknown persisted status: {}", other)),
        };

        let created_at = if persisted.created_at.is_empty() {
            Utc::now()
        } else {
            Utc::now()
        };

        let finished_at = persisted
            .finished_at
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));

        let instance = SubagentInstance {
            id,
            name: persisted.name,
            task_description: persisted.task,
            agent_loop: AgentLoop::new(model_cfg),
            status,
            cancellation_token: CancellationToken::new(),
            created_at,
            finished_at,
        };

        let mut guard = self.instances.write().await;
        guard.insert(persisted.id, instance);

        Ok(())
    }

    async fn persist_status(&self, instance: &SubagentInstance) -> Result<()> {
        if let Some(ref persistence) = self.persistence {
            let persisted = Self::instance_to_persisted(instance);
            persistence.save(&persisted).await?;
        }
        Ok(())
    }

    fn instance_to_persisted(instance: &SubagentInstance) -> PersistedSubagent {
        let (status_str, output_str) = match &instance.status {
            SubagentStatus::Running => ("Running".to_string(), None),
            SubagentStatus::Idle => ("Idle".to_string(), None),
            SubagentStatus::Finished(output) => ("Finished".to_string(), Some(output.clone())),
            SubagentStatus::Errored(error) => ("Errored".to_string(), Some(error.clone())),
        };

        PersistedSubagent {
            id: instance.id.to_string(),
            name: instance.name.clone(),
            task: instance.task_description.clone(),
            status: status_str,
            output: output_str,
            created_at: instance.created_at.to_rfc3339(),
            finished_at: instance.finished_at.map(|t| t.to_rfc3339()),
            transcript_path: None,
            model_config: Some(ModelConfigSnapshot {
                provider: instance.agent_loop.model_config.provider.clone(),
                model_id: instance.agent_loop.model_config.model_id.clone(),
                api_key: Some(instance.agent_loop.model_config.api_key.clone()),
                base_url: instance.agent_loop.model_config.base_url.clone(),
                context_window: instance.agent_loop.model_config.context_window,
                max_output: instance.agent_loop.model_config.max_output,
            }),
        }
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
        };

        self.persist_status(&instance).await?;

        {
            let mut guard = self.instances.write().await;
            guard.insert(id_str.clone(), instance);
        }

        let instances_clone = self.instances.clone();
        let id_clone = id_str.clone();
        let task_owned = task.to_string();
        let persistence_clone = self.persistence.clone();

        tokio::spawn(async move {
            let result =
                SubagentRunner::run_task(child_agent_loop, task_owned, cancellation_token).await;

            let mut guard = instances_clone.write().await;
            if let Some(inst) = guard.get_mut(&id_clone) {
                inst.finished_at = Some(Utc::now());
                match result {
                    Ok(output) => inst.status = SubagentStatus::Finished(output),
                    Err(err) => inst.status = SubagentStatus::Errored(err.to_string()),
                }

                if let Some(ref persistence) = persistence_clone {
                    let persisted = Self::instance_to_persisted(inst);
                    let _ = persistence.save(&persisted).await;
                }
            }
        });

        Ok(id_str)
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
        };

        self.persist_status(&instance).await?;

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
                Ok(output) => inst.status = SubagentStatus::Finished(output.clone()),
                Err(err) => inst.status = SubagentStatus::Errored(err.to_string()),
            }
            let _ = self.persist_status(inst).await;
        }

        result
    }

    pub async fn resume(&self, id_or_name: &str, followup_task: &str) -> Result<String> {
        let persisted = match self.persistence.as_ref() {
            Some(persistence) => persistence.load_by_id_or_name(id_or_name).await.ok().flatten(),
            None => None,
        };
        let persisted = persisted
            .ok_or_else(|| anyhow!("No persisted subagent found for '{}'", id_or_name))?;

        let subagent_id = Uuid::new_v4();
        let id_str = subagent_id.to_string();

        let model_cfg = if let Some(ref model_snap) = persisted.model_config {
            ModelConfig {
                provider: model_snap.provider.clone(),
                model_id: model_snap.model_id.clone(),
                api_key: model_snap.api_key.clone().unwrap_or_default(),
                base_url: model_snap.base_url.clone(),
                context_window: model_snap.context_window,
                max_output: model_snap.max_output,
            }
        } else {
            self.default_model_config.clone()
        };

        let mut child_agent_loop = AgentLoop::new(model_cfg);
        let summary = if let Some(ref history) = persisted.output {
            format!(
                "Previous task: {}\nPrevious output:\n{}\n\nFollow-up task: {}",
                persisted.task, history, followup_task
            )
        } else {
            format!(
                "Previous task: {}\n\nFollow-up task: {}",
                persisted.task, followup_task
            )
        };
        child_agent_loop
            .session_tree
            .append_child(pi_session::Role::User, summary);

        let cancellation_token = CancellationToken::new();

        let instance = SubagentInstance {
            id: subagent_id,
            name: persisted.name.clone(),
            task_description: format!("{} (resumed)", followup_task),
            agent_loop: child_agent_loop.clone(),
            status: SubagentStatus::Running,
            cancellation_token: cancellation_token.clone(),
            created_at: Utc::now(),
            finished_at: None,
        };

        self.persist_status(&instance).await?;

        {
            let mut guard = self.instances.write().await;
            guard.insert(id_str.clone(), instance);
        }

        let instances_clone = self.instances.clone();
        let persistence_clone = self.persistence.clone();

        let followup_owned = followup_task.to_string();
        let id_spawn = id_str.clone();
        tokio::spawn(async move {
            let result = SubagentRunner::run_task(
                child_agent_loop,
                followup_owned,
                cancellation_token,
            )
            .await;

            let mut guard = instances_clone.write().await;
            if let Some(inst) = guard.get_mut(&id_spawn) {
                inst.finished_at = Some(Utc::now());
                match result {
                    Ok(output) => inst.status = SubagentStatus::Finished(output),
                    Err(err) => inst.status = SubagentStatus::Errored(err.to_string()),
                }

                if let Some(ref persistence) = persistence_clone {
                    let persisted = Self::instance_to_persisted(inst);
                    let _ = persistence.save(&persisted).await;
                }
            }
        });

        Ok(id_str)
    }

    pub async fn load_persisted_subagent(
        &self,
        id_or_name: &str,
    ) -> Result<Option<PersistedSubagent>> {
        if let Some(ref persistence) = self.persistence {
            let by_id = persistence.load(id_or_name).await?;
            if by_id.is_some() {
                return Ok(by_id);
            }

            let all = persistence.list().await;
            let found = all
                .into_iter()
                .find(|p| p.id == id_or_name || p.name == id_or_name);
            return Ok(found);
        }

        Ok(None)
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
            let _ = self.persist_status(inst).await;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subagent_persistence::SubagentPersistence;

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

        let killed = manager.kill(&id).await.unwrap();
        assert!(killed);

        let status = manager.get_status(&id).await.unwrap();
        assert!(matches!(status, SubagentStatus::Errored(_)));
    }

    #[tokio::test]
    async fn test_subagent_persistence_on_spawn() {
        let tmp = tempfile::tempdir().unwrap();
        let persistence = SubagentPersistence::new(tmp.path()).unwrap();
        let manager = Arc::new(
            SubagentManager::new_with_empty_model()
                .with_persistence(persistence.clone()),
        );

        let config = SubagentConfig {
            name: "PersistWorker".to_string(),
            model_override: None,
            system_prompt_override: Some("You are a persistent worker".to_string()),
            allowed_tools: None,
        };

        let id = manager.spawn(config, "Persist this task").await.unwrap();
        manager.kill(&id).await.unwrap();

        let loaded = persistence.load(&id).await.unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.name, "PersistWorker");
        assert_eq!(loaded.task, "Persist this task");
    }

    #[tokio::test]
    async fn test_subagent_persistence_survives_restart() {
        let tmp = tempfile::tempdir().unwrap();

        {
            let persistence = SubagentPersistence::new(tmp.path()).unwrap();
            let manager = SubagentManager::new_with_empty_model()
                .with_persistence(persistence.clone());

            let config = SubagentConfig {
                name: "RestartWorker".to_string(),
                model_override: None,
                system_prompt_override: Some("You are a restart worker".to_string()),
                allowed_tools: None,
            };

            let id = manager.spawn(config, "Survive restart").await.unwrap();
            manager.kill(&id).await.unwrap();
        }

        let reloaded = SubagentManager::load_persisted(
            ModelConfig::resolve("mock/mock-model"),
            tmp.path(),
        )
        .await
        .unwrap();

        let list = reloaded.list().await;
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "RestartWorker");
        assert_eq!(list[0].task, "Survive restart");
        assert!(list[0].status.contains("Errored") || list[0].status.contains("cancelled"));
    }

    #[tokio::test]
    async fn test_subagent_resume_seeds_context() {
        let tmp = tempfile::tempdir().unwrap();

        {
            let persistence = SubagentPersistence::new(tmp.path()).unwrap();
            let manager = SubagentManager::new_with_empty_model()
                .with_persistence(persistence.clone());

            let config = SubagentConfig {
                name: "ResumeWorker".to_string(),
                model_override: None,
                system_prompt_override: Some("You are a resumable worker".to_string()),
                allowed_tools: None,
            };

            let id = manager.spawn(config, "Original task").await.unwrap();
            manager.kill(&id).await.unwrap();
        }

        let manager = SubagentManager::load_persisted(
            ModelConfig::resolve("mock/mock-model"),
            tmp.path(),
        )
        .await
        .unwrap();

        let resume_id = manager
            .resume("ResumeWorker", "Follow-up task")
            .await
            .unwrap();

        let list = manager.list().await;
        let resumed = list.iter().find(|item| item.id == resume_id);
        assert!(resumed.is_some());
        assert_eq!(resumed.unwrap().name, "ResumeWorker");
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
}
