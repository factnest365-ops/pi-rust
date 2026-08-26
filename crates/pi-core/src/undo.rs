use anyhow::{Result, anyhow};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionSnapshotKind {
    FileWrite,
    FileEdit,
    FileDelete,
    CommandExecution,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionSnapshot {
    pub id: String,
    pub timestamp: i64,
    pub description: String,
    pub kind: ActionSnapshotKind,
    pub path: Option<PathBuf>,
    pub pre_content: Option<String>,
    pub post_content: Option<String>,
    pub is_undone: bool,
}

#[derive(Debug, Default)]
pub struct UndoEngine {
    history: Vec<ActionSnapshot>,
}

impl UndoEngine {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
        }
    }

    /// Records a new action snapshot into the undo journal.
    pub fn record_action(
        &mut self,
        description: &str,
        kind: ActionSnapshotKind,
        path: Option<&Path>,
        pre_content: Option<&str>,
        post_content: Option<&str>,
    ) -> String {
        let id = Uuid::new_v4().to_string();
        let snapshot = ActionSnapshot {
            id: id.clone(),
            timestamp: Utc::now().timestamp(),
            description: description.to_string(),
            kind,
            path: path.map(|p| p.to_path_buf()),
            pre_content: pre_content.map(|s| s.to_string()),
            post_content: post_content.map(|s| s.to_string()),
            is_undone: false,
        };
        self.history.push(snapshot);
        id
    }

    /// Captures a file state before an edit/write.
    pub fn capture_file_pre_state(path: &Path) -> Option<String> {
        if path.exists() {
            fs::read_to_string(path).ok()
        } else {
            None
        }
    }

    /// Returns a list of all recorded action snapshots.
    pub fn history(&self) -> &[ActionSnapshot] {
        &self.history
    }

    /// Returns the count of active, reversible snapshots.
    pub fn reversible_count(&self) -> usize {
        self.history.iter().filter(|s| !s.is_undone).count()
    }

    /// Previews the diff of what undoing a specific action will do.
    pub fn preview_undo(&self, action_id: &str) -> Result<String> {
        let snapshot = self
            .history
            .iter()
            .find(|s| s.id == action_id)
            .ok_or_else(|| anyhow!("Action snapshot with ID '{}' not found", action_id))?;

        if snapshot.is_undone {
            return Ok(format!(
                "Action '{}' has already been rolled back.",
                snapshot.description
            ));
        }

        let mut diff_preview = String::new();
        diff_preview.push_str(&format!("### Rollback Preview: {}\n", snapshot.description));
        diff_preview.push_str(&format!("**Action Type:** {:?}\n", snapshot.kind));

        if let Some(path) = &snapshot.path {
            diff_preview.push_str(&format!("**Target Path:** {}\n\n", path.display()));
        }

        match snapshot.kind {
            ActionSnapshotKind::FileWrite => {
                if snapshot.pre_content.is_none() {
                    diff_preview.push_str("*(Rollback will delete the newly created file)*\n");
                } else {
                    diff_preview.push_str("*(Rollback will restore previous file content)*\n");
                }
            }
            ActionSnapshotKind::FileEdit => {
                diff_preview.push_str("```diff\n");
                if let Some(post) = &snapshot.post_content {
                    for line in post.lines() {
                        diff_preview.push_str(&format!("- {}\n", line));
                    }
                }
                if let Some(pre) = &snapshot.pre_content {
                    for line in pre.lines() {
                        diff_preview.push_str(&format!("+ {}\n", line));
                    }
                }
                diff_preview.push_str("```\n");
            }
            ActionSnapshotKind::FileDelete => {
                diff_preview.push_str("*(Rollback will re-create the deleted file)*\n");
            }
            ActionSnapshotKind::CommandExecution => {
                diff_preview.push_str("*(Command executions may have external side effects and cannot be guaranteed fully reversible)*\n");
            }
        }

        Ok(diff_preview)
    }

    /// Undoes a specific action by ID.
    pub fn undo_by_id(&mut self, action_id: &str) -> Result<String> {
        let snapshot = self
            .history
            .iter_mut()
            .find(|s| s.id == action_id)
            .ok_or_else(|| anyhow!("Action snapshot with ID '{}' not found", action_id))?;

        if snapshot.is_undone {
            return Ok(format!(
                "Action '{}' is already rolled back.",
                snapshot.description
            ));
        }

        Self::apply_rollback(snapshot)?;
        snapshot.is_undone = true;

        Ok(format!(
            "Successfully rolled back: {}",
            snapshot.description
        ))
    }

    /// Undoes the last `n` active actions in reverse chronological order.
    pub fn undo_last(&mut self, n: usize) -> Result<Vec<String>> {
        let mut results = Vec::new();
        let mut count = 0;

        for snapshot in self.history.iter_mut().rev() {
            if count >= n {
                break;
            }
            if !snapshot.is_undone {
                Self::apply_rollback(snapshot)?;
                snapshot.is_undone = true;
                results.push(format!("Rolled back: {}", snapshot.description));
                count += 1;
            }
        }

        if results.is_empty() {
            return Err(anyhow!("No active actions available to undo"));
        }

        Ok(results)
    }

    fn apply_rollback(snapshot: &ActionSnapshot) -> Result<()> {
        if let Some(path) = &snapshot.path {
            match snapshot.kind {
                ActionSnapshotKind::FileWrite => {
                    if let Some(pre) = &snapshot.pre_content {
                        if let Some(parent) = path.parent() {
                            fs::create_dir_all(parent)?;
                        }
                        fs::write(path, pre)?;
                    } else if path.exists() {
                        fs::remove_file(path)?;
                    }
                }
                ActionSnapshotKind::FileEdit => {
                    if let Some(pre) = &snapshot.pre_content {
                        if let Some(parent) = path.parent() {
                            fs::create_dir_all(parent)?;
                        }
                        fs::write(path, pre)?;
                    } else if path.exists() {
                        fs::remove_file(path)?;
                    }
                }
                ActionSnapshotKind::FileDelete => {
                    if let Some(pre) = &snapshot.pre_content {
                        if let Some(parent) = path.parent() {
                            fs::create_dir_all(parent)?;
                        }
                        fs::write(path, pre)?;
                    }
                }
                ActionSnapshotKind::CommandExecution => {
                    // Command executions log a warning on rollback
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_undo_file_write_new_file() {
        let tmp = tempdir().unwrap();
        let file_path = tmp.path().join("test.txt");

        let mut engine = UndoEngine::new();

        // 1. File written
        fs::write(&file_path, "Hello world").unwrap();
        let id = engine.record_action(
            "Create test.txt",
            ActionSnapshotKind::FileWrite,
            Some(&file_path),
            None,
            Some("Hello world"),
        );

        assert!(file_path.exists());
        assert_eq!(engine.reversible_count(), 1);

        // 2. Undo
        let res = engine.undo_by_id(&id).unwrap();
        assert!(res.contains("Successfully rolled back"));
        assert!(!file_path.exists());
        assert_eq!(engine.reversible_count(), 0);
    }

    #[test]
    fn test_undo_file_edit() {
        let tmp = tempdir().unwrap();
        let file_path = tmp.path().join("code.rs");

        fs::write(&file_path, "pub fn original() {}").unwrap();

        let mut engine = UndoEngine::new();

        // Edit
        fs::write(&file_path, "pub fn modified() {}").unwrap();
        let id = engine.record_action(
            "Refactor code.rs",
            ActionSnapshotKind::FileEdit,
            Some(&file_path),
            Some("pub fn original() {}"),
            Some("pub fn modified() {}"),
        );

        assert_eq!(
            fs::read_to_string(&file_path).unwrap(),
            "pub fn modified() {}"
        );

        // Preview diff
        let preview = engine.preview_undo(&id).unwrap();
        assert!(preview.contains("+ pub fn original() {}"));
        assert!(preview.contains("- pub fn modified() {}"));

        // Rollback
        engine.undo_by_id(&id).unwrap();
        assert_eq!(
            fs::read_to_string(&file_path).unwrap(),
            "pub fn original() {}"
        );
    }

    #[test]
    fn test_undo_last_multiple() {
        let tmp = tempdir().unwrap();
        let f1 = tmp.path().join("f1.txt");
        let f2 = tmp.path().join("f2.txt");

        let mut engine = UndoEngine::new();

        fs::write(&f1, "1").unwrap();
        engine.record_action(
            "Write f1",
            ActionSnapshotKind::FileWrite,
            Some(&f1),
            None,
            Some("1"),
        );

        fs::write(&f2, "2").unwrap();
        engine.record_action(
            "Write f2",
            ActionSnapshotKind::FileWrite,
            Some(&f2),
            None,
            Some("2"),
        );

        assert_eq!(engine.reversible_count(), 2);

        let undone = engine.undo_last(2).unwrap();
        assert_eq!(undone.len(), 2);
        assert!(!f1.exists());
        assert!(!f2.exists());
        assert_eq!(engine.reversible_count(), 0);
    }
}
