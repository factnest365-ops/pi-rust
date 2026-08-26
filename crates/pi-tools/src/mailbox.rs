//! A2A mailbox: model-facing send/inbox/mark-read tools.
//!
//! Self-contained in pi-tools so the tool registry needs no core dep.
//! A JSONL-backed shared store lives at <data_dir>/mailboxes/mailbox.jsonl;
//! tests can inject a custom path via `set_store_path`.

use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::RwLock;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MailMessage {
    pub id: String,
    pub from: String,
    pub to: String,
    pub content: Value,
    pub read: bool,
    pub created_at: String,
}

static MAILBOX: RwLock<Vec<MailMessage>> = RwLock::new(Vec::new());
static STORE_PATH: RwLock<Option<PathBuf>> = RwLock::new(None);
static SEQ: AtomicUsize = AtomicUsize::new(1);

fn next_id() -> String {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    format!("msg-{n:06}")
}

fn store_path() -> Option<PathBuf> {
    STORE_PATH.read().ok().and_then(|guard| guard.clone())
}

/// Override the persistence path (used by host startup and tests).
pub fn set_store_path(path: PathBuf) {
    if let Ok(mut guard) = STORE_PATH.write() {
        *guard = Some(path);
    }
}

fn load_all() -> Vec<MailMessage> {
    let mut msgs = Vec::new();
    if let Some(path) = store_path()
        && let Ok(text) = std::fs::read_to_string(&path)
    {
        for line in text.lines() {
            if let Ok(m) = serde_json::from_str::<MailMessage>(line) {
                msgs.push(m);
            }
        }
    }
    // Overlay anything sent this session not yet flushed (id dedupe).
    if let Ok(guard) = MAILBOX.read() {
        for m in guard.iter() {
            if !msgs.iter().any(|x| x.id == m.id) {
                msgs.push(m.clone());
            }
        }
    }
    msgs
}

fn rewrite_store(msgs: &[MailMessage]) {
    if let Some(path) = store_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut body = String::new();
        for m in msgs {
            body.push_str(&serde_json::to_string(m).unwrap_or_default());
            body.push('\n');
        }
        let _ = std::fs::write(&path, body);
    }
}

fn persist(msg: &MailMessage) {
    use std::io::Write;
    if let Ok(mut guard) = MAILBOX.write() {
        guard.push(msg.clone())
    }
    if let Some(path) = store_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let _ = writeln!(f, "{}", serde_json::to_string(msg).unwrap_or_default());
        }
    }
}

pub struct MailboxTools;

impl MailboxTools {
    pub fn execute_send(args: &Value) -> Result<Value, anyhow::Error> {
        let to = args
            .get("to")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("agent_send requires {{to}}"))?;
        let from = args.get("from").and_then(Value::as_str).unwrap_or("main");
        let content = args
            .get("content")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("agent_send requires {{content}}"))?;
        let msg = MailMessage {
            id: next_id(),
            from: from.to_string(),
            to: to.to_string(),
            content,
            read: false,
            created_at: {
                let now: std::time::SystemTime = std::time::SystemTime::now();
                let secs = now
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                format!("{secs}")
            },
        };
        persist(&msg);
        Ok(json!({ "sent": true, "id": msg.id }))
    }

    pub fn execute_inbox(args: &Value) -> Result<Value, anyhow::Error> {
        let agent = args.get("agent").and_then(Value::as_str).unwrap_or("main");
        let unread_only = args
            .get("unread_only")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let messages: Vec<MailMessage> = load_all()
            .into_iter()
            .filter(|m| m.to == agent && (!unread_only || !m.read))
            .collect();
        Ok(json!({ "messages": messages }))
    }

    pub fn execute_mark_read(args: &Value) -> Result<Value, anyhow::Error> {
        let agent = args.get("agent").and_then(Value::as_str).unwrap_or("main");
        let ids: Vec<String> = args
            .get("ids")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        if ids.is_empty() {
            return Err(anyhow::anyhow!(
                "agent_mark_read requires non-empty {{ids}}"
            ));
        }
        let mut count = 0;
        let mut all = load_all();
        for m in all.iter_mut() {
            if m.to == agent && ids.contains(&m.id) && !m.read {
                m.read = true;
                count += 1;
            }
        }
        if count > 0 {
            if let Ok(mut guard) = MAILBOX.write() {
                *guard = all.clone();
            }
            rewrite_store(&all);
        }
        Ok(json!({ "marked_read": count }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_inbox_markread_roundtrip() {
        set_store_path(std::env::temp_dir().join(format!("tau-mb-test-{}", std::process::id())));
        let r = MailboxTools::execute_send(
            &json!({"to":"scout","from":"planner","content":{"task":"go"}}),
        )
        .unwrap();
        assert_eq!(r["sent"], true);
        let inbox = MailboxTools::execute_inbox(&json!({"agent":"scout"})).unwrap();
        assert_eq!(inbox["messages"].as_array().unwrap().len(), 1);
        let id = inbox["messages"][0]["id"].as_str().unwrap().to_string();
        let marked = MailboxTools::execute_mark_read(&json!({"agent":"scout","ids":[id]})).unwrap();
        assert_eq!(marked["marked_read"], 1);
        let inbox2 = MailboxTools::execute_inbox(&json!({"agent":"scout"})).unwrap();
        assert_eq!(inbox2["messages"].as_array().unwrap().len(), 0);
    }
}
