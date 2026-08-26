use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedSubagent {
    pub id: String,
    pub name: String,
    pub task: String,
    pub status: String,
    pub output: Option<String>,
    pub created_at: String,
    pub finished_at: Option<String>,
    pub transcript_path: Option<String>,
    pub model_config: Option<ModelConfigSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfigSnapshot {
    pub provider: String,
    pub model_id: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub context_window: usize,
    pub max_output: usize,
}

#[derive(Clone)]
pub struct SubagentPersistence {
    data_dir: PathBuf,
    subagents_dir: PathBuf,
    instances: Arc<RwLock<HashMap<String, PersistedSubagent>>>,
}

impl SubagentPersistence {
    pub fn new<P: AsRef<Path>>(data_dir: P) -> Result<Self> {
        let data_dir = data_dir.as_ref().to_path_buf();
        let subagents_dir = data_dir.join("subagents");
        fs::create_dir_all(&subagents_dir).context("Failed to create subagents directory")?;

        let persistence = Self {
            data_dir,
            subagents_dir,
            instances: Arc::new(RwLock::new(HashMap::new())),
        };

        persistence.load_all_async();

        Ok(persistence)
    }

    pub fn new_in_memory() -> Self {
        Self {
            data_dir: PathBuf::from("/tmp/pi-subagents-memory"),
            subagents_dir: PathBuf::from("/tmp/pi-subagents-memory/subagents"),
            instances: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn save(&self, persisted: &PersistedSubagent) -> Result<()> {
        let path = self.subagents_dir.join(format!("{}.json", persisted.id));
        let tmp_path = self.subagents_dir.join(format!("{}.json.tmp", persisted.id));

        let json = serde_json::to_string_pretty(persisted).context("Failed to serialize subagent")?;
        fs::write(&tmp_path, json).context("Failed to write temporary subagent file")?;
        fs::rename(&tmp_path, &path).context("Failed to atomically rename subagent file")?;

        let mut instances = self.instances.write().await;
        instances.insert(persisted.id.clone(), persisted.clone());

        Ok(())
    }

    pub async fn load(&self, id: &str) -> Result<Option<PersistedSubagent>> {
        {
            let instances = self.instances.read().await;
            if let Some(persisted) = instances.get(id) {
                return Ok(Some(persisted.clone()));
            }
        }

        let path = self.subagents_dir.join(format!("{}.json", id));
        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&path).context("Failed to read subagent file")?;
        let persisted: PersistedSubagent =
            serde_json::from_str(&content).context("Failed to parse subagent JSON")?;

        let mut instances = self.instances.write().await;
        instances.insert(id.to_string(), persisted.clone());

        Ok(Some(persisted))
    }

    /// Find one persisted subagent by id or name, scanning disk on miss.
    pub async fn load_by_id_or_name(&self, id_or_name: &str) -> Result<Option<PersistedSubagent>> {
        if let Some(found) = self.load(id_or_name).await? {
            return Ok(Some(found));
        }
        for persisted in self.list().await {
            if persisted.name == id_or_name {
                return Ok(Some(persisted));
            }
        }
        Ok(None)
    }

    pub fn load_all_async(&self) {
        if !self.subagents_dir.exists() {
            return;
        }

        let instances = self.instances.clone();
        let subagents_dir = self.subagents_dir.clone();

        tokio::spawn(async move {
            if let Ok(entries) = fs::read_dir(&subagents_dir) {
                let mut guard = instances.write().await;
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("json") {
                        if let Ok(content) = fs::read_to_string(&path) {
                            if let Ok(persisted) =
                                serde_json::from_str::<PersistedSubagent>(&content)
                            {
                                guard.insert(persisted.id.clone(), persisted);
                            }
                        }
                    }
                }
            }
        });
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let path = self.subagents_dir.join(format!("{}.json", id));
        if path.exists() {
            fs::remove_file(&path).context("Failed to delete subagent file")?;
        }
        let mut instances = self.instances.write().await;
        instances.remove(id);
        Ok(())
    }

    pub async fn list(&self) -> Vec<PersistedSubagent> {
        {
            let instances = self.instances.read().await;
            if !instances.is_empty() {
                return instances.values().cloned().collect();
            }
        }
        // In-memory cache empty: scan the on-disk store so a fresh process
        // (restart) sees previously persisted subagents.
        let mut out = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.subagents_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Ok(text) = std::fs::read_to_string(&path) {
                        if let Ok(persisted) = serde_json::from_str::<PersistedSubagent>(&text) {
                            out.push(persisted);
                        }
                    }
                }
            }
        }
        out
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn subagents_dir(&self) -> &Path {
        &self.subagents_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_persistence_save_and_load() {
        let tmp = tempfile::tempdir().unwrap();
        let persistence = SubagentPersistence::new(tmp.path()).unwrap();

        let persisted = PersistedSubagent {
            id: "test-123".to_string(),
            name: "TestAgent".to_string(),
            task: "Do something".to_string(),
            status: "Running".to_string(),
            output: None,
            created_at: "2026-08-26T10:00:00Z".to_string(),
            finished_at: None,
            transcript_path: None,
            model_config: Some(ModelConfigSnapshot {
                provider: "openai".to_string(),
                model_id: "gpt-4".to_string(),
                api_key: Some("test-key".to_string()),
                base_url: None,
                context_window: 128000,
                max_output: 8192,
            }),
        };

        persistence.save(&persisted).await.unwrap();
        let loaded = persistence.load("test-123").await.unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.id, "test-123");
        assert_eq!(loaded.name, "TestAgent");
        assert_eq!(loaded.task, "Do something");
        assert_eq!(loaded.status, "Running");

        let path = tmp.path().join("subagents").join("test-123.json");
        assert!(path.exists());
    }

    #[tokio::test]
    async fn test_persistence_atomic_write() {
        let tmp = tempfile::tempdir().unwrap();
        let persistence = SubagentPersistence::new(tmp.path()).unwrap();

        let persisted = PersistedSubagent {
            id: "atomic-test".to_string(),
            name: "AtomicAgent".to_string(),
            task: "Test atomic write".to_string(),
            status: "Finished".to_string(),
            output: Some("Done".to_string()),
            created_at: "2026-08-26T10:00:00Z".to_string(),
            finished_at: Some("2026-08-26T10:05:00Z".to_string()),
            transcript_path: None,
            model_config: None,
        };

        persistence.save(&persisted).await.unwrap();

        let subagents_dir = tmp.path().join("subagents");
        for entry in fs::read_dir(&subagents_dir).unwrap() {
            let path = entry.unwrap().path();
            assert!(path.extension().and_then(|e| e.to_str()) != Some("tmp"));
        }

        let path = subagents_dir.join("atomic-test.json");
        let content = fs::read_to_string(&path).unwrap();
        let loaded: PersistedSubagent = serde_json::from_str(&content).unwrap();
        assert_eq!(loaded.status, "Finished");
        assert_eq!(loaded.output, Some("Done".to_string()));
    }

    #[tokio::test]
    async fn test_persistence_load_nonexistent() {
        let tmp = tempfile::tempdir().unwrap();
        let persistence = SubagentPersistence::new(tmp.path()).unwrap();
        let loaded = persistence.load("nonexistent").await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_persistence_delete() {
        let tmp = tempfile::tempdir().unwrap();
        let persistence = SubagentPersistence::new(tmp.path()).unwrap();

        let persisted = PersistedSubagent {
            id: "delete-me".to_string(),
            name: "DeleteAgent".to_string(),
            task: "To be deleted".to_string(),
            status: "Idle".to_string(),
            output: None,
            created_at: "2026-08-26T10:00:00Z".to_string(),
            finished_at: None,
            transcript_path: None,
            model_config: None,
        };

        persistence.save(&persisted).await.unwrap();
        assert!(persistence.load("delete-me").await.unwrap().is_some());

        persistence.delete("delete-me").await.unwrap();
        assert!(persistence.load("delete-me").await.unwrap().is_none());

        let path = tmp.path().join("subagents").join("delete-me.json");
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn test_persistence_list() {
        let tmp = tempfile::tempdir().unwrap();
        let persistence = SubagentPersistence::new(tmp.path()).unwrap();

        for i in 1..=3 {
            let persisted = PersistedSubagent {
                id: format!("agent-{}", i),
                name: format!("Agent{}", i),
                task: format!("Task {}", i),
                status: "Idle".to_string(),
                output: None,
                created_at: "2026-08-26T10:00:00Z".to_string(),
                finished_at: None,
                transcript_path: None,
                model_config: None,
            };
            persistence.save(&persisted).await.unwrap();
        }

        let list = persistence.list().await;
        assert_eq!(list.len(), 3);
        let ids: Vec<String> = list.iter().map(|p| p.id.clone()).collect();
        assert!(ids.contains(&"agent-1".to_string()));
        assert!(ids.contains(&"agent-2".to_string()));
        assert!(ids.contains(&"agent-3".to_string()));
    }

    #[tokio::test]
    async fn test_persistence_survives_drop_recreate() {
        let tmp = tempfile::tempdir().unwrap();
        let subagents_dir = tmp.path().join("subagents");
        fs::create_dir_all(&subagents_dir).unwrap();

        {
            let persistence = SubagentPersistence::new(tmp.path()).unwrap();
            let persisted = PersistedSubagent {
                id: "survive-test".to_string(),
                name: "SurviveAgent".to_string(),
                task: "Survive restart".to_string(),
                status: "Finished".to_string(),
                output: Some("Previous output".to_string()),
                created_at: "2026-08-26T10:00:00Z".to_string(),
                finished_at: Some("2026-08-26T10:05:00Z".to_string()),
                transcript_path: None,
                model_config: None,
            };
            persistence.save(&persisted).await.unwrap();
        }

        let persistence2 = SubagentPersistence::new(tmp.path()).unwrap();
        let loaded = persistence2.load("survive-test").await.unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.name, "SurviveAgent");
        assert_eq!(loaded.status, "Finished");
        assert_eq!(loaded.output, Some("Previous output".to_string()));
    }
}
