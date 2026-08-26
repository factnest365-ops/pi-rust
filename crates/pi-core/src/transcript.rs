use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubagentTranscript {
    pub id: String,
    pub name: String,
    pub task: String,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub history: Vec<TranscriptNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranscriptNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub role: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

impl SubagentTranscript {
    pub fn new(id: String, name: String, task: String) -> Self {
        let task_summary = task.clone();
        let now = Utc::now();
        Self {
            id,
            name,
            task,
            created_at: now,
            finished_at: None,
            history: vec![TranscriptNode {
                id: Uuid::new_v4().to_string(),
                parent_id: None,
                role: "system".to_string(),
                content: format!("Subagent transcript started for task: {}", task_summary),
                timestamp: now,
            }],
        }
    }

    pub fn push_user(&mut self, content: &str) -> String {
        let node_id = Uuid::new_v4().to_string();
        let parent_id = self.history.last().map(|node| node.id.clone());
        self.history.push(TranscriptNode {
            id: node_id.clone(),
            parent_id,
            role: "user".to_string(),
            content: content.to_string(),
            timestamp: Utc::now(),
        });
        node_id
    }

    pub fn push_assistant(&mut self, content: &str) -> String {
        let node_id = Uuid::new_v4().to_string();
        let parent_id = self.history.last().map(|node| node.id.clone());
        self.history.push(TranscriptNode {
            id: node_id.clone(),
            parent_id,
            role: "assistant".to_string(),
            content: content.to_string(),
            timestamp: Utc::now(),
        });
        node_id
    }

    pub fn finish(&mut self) {
        self.finished_at = Some(Utc::now());
    }

    pub fn summary(&self) -> String {
        let mut summary = format!(
            "Previous task: {}\nAgent: {}\nHistory:\n",
            self.task, self.name
        );

        for node in &self.history {
            let truncated = if node.content.len() > 1000 {
                format!("{}...", &node.content[..1000])
            } else {
                node.content.clone()
            };
            summary.push_str(&format!("- [{}] {}: {}\n", node.role, node.id, truncated));
        }

        if let Some(last) = self.history.last() {
            summary.push_str(&format!("\nLast assistant response: {}\n", last.content));
        }

        summary
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub id: String,
    pub from: String,
    pub to: String,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

impl AgentMessage {
    pub fn new(from: impl Into<String>, to: impl Into<String>, payload: Value) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            from: from.into(),
            to: to.into(),
            payload,
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AgentMailbox {
    inbox: Vec<AgentMessage>,
    sent: Vec<AgentMessage>,
}

impl AgentMailbox {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn deliver(&mut self, message: AgentMessage) {
        self.inbox.push(message);
    }

    pub fn receive(&self, agent: &str, unread_only: bool) -> Vec<&AgentMessage> {
        if unread_only {
            self.inbox
                .iter()
                .filter(|message| message.to == agent)
                .collect()
        } else {
            self.inbox.iter().collect()
        }
    }

    pub fn send(&mut self, message: AgentMessage) {
        self.sent.push(message.clone());
        self.inbox.push(message);
    }

    pub fn mark_read(&mut self, agent: &str, ids: &[String]) {
        for message in &mut self.inbox {
            if message.to == agent && ids.contains(&message.id) {
                message.payload["read"] = Value::Bool(true);
            }
        }
    }

    pub fn sent_by(&self, agent: &str) -> Vec<&AgentMessage> {
        self.sent
            .iter()
            .filter(|message| message.from == agent)
            .collect()
    }
}

pub fn append_only_jsonl<P: AsRef<Path>>(path: P, line: &str) -> std::io::Result<()> {
    use std::fs::OpenOptions;
    use std::io::Write;

    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{}", line)?;
    Ok(())
}

pub fn atomic_jsonl_write<P: AsRef<Path>>(path: P, payload: &Value) -> std::io::Result<()> {
    let path = path.as_ref();
    let tmp_path = path.with_extension("jsonl.tmp");

    let line = payload.to_string();
    std::fs::write(&tmp_path, format!("{}\n", line))?;
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}

pub fn system_timestamp_now() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| format!("{}", duration.as_millis()))
        .unwrap_or_else(|_| "0".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_subagent_transcript_roundtrip() {
        let mut transcript =
            SubagentTranscript::new("sub-1".into(), "Reviewer".into(), "Review PR".into());
        let _user_id = transcript.push_user("Please review src/lib.rs");
        transcript.push_assistant("I will inspect the file.");
        transcript.finish();

        let json = serde_json::to_string(&transcript).unwrap();
        let loaded: SubagentTranscript = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.history.len(), 3);
        assert_eq!(loaded.id, "sub-1");
        assert_eq!(loaded.name, "Reviewer");
        assert_eq!(loaded.task, "Review PR");
    }

    #[test]
    fn test_agent_message_delivery() {
        let mut mailbox = AgentMailbox::new();
        let message = AgentMessage::new("bob", "alice", serde_json::json!({"text": "hello"}));
        mailbox.deliver(message);

        let received = mailbox.receive("alice", true);
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].from, "bob");
        assert_eq!(received[0].payload["text"], "hello");
    }

    #[test]
    fn test_mailbox_mark_read_and_sent() {
        let mut mailbox = AgentMailbox::new();
        let message = AgentMessage::new("bob", "alice", serde_json::json!({"text": "hello"}));
        mailbox.send(message);

        mailbox.mark_read("alice", &["ignored".into(), "does-not-matter".into()]);
        let received = mailbox.receive("alice", true);
        assert_eq!(received.len(), 1);
        let sent = mailbox.sent_by("bob");
        assert_eq!(sent.len(), 1);
    }

    #[test]
    fn test_atomic_jsonl_write_uses_temp_then_rename() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("events.jsonl");

        let payload = serde_json::json!({"event": "hello"});
        atomic_jsonl_write(&target, &payload).unwrap();

        assert!(target.exists());
        assert!(!target.with_extension("jsonl.tmp").exists());
        let contents = fs::read_to_string(target).unwrap();
        assert!(contents.contains("\"event\":\"hello\""));
    }

    #[test]
    fn test_system_timestamp_format() {
        let timestamp = system_timestamp_now();
        assert!(timestamp.parse::<u128>().is_ok());
        assert!(timestamp.len() > 10);
    }
}
