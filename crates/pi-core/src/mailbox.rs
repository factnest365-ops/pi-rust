//! Process-global A2A mailbox registry.
//!
//! Wraps `transcript::AgentMailbox` in a Mutex so senders can mutate the
//! shared store. Hosts call [`MailboxRegistry::init`] once at startup.

use crate::transcript::{AgentMailbox, AgentMessage};
use serde_json::Value;
use std::sync::{Arc, Mutex, RwLock};

static MAILBOX_STORE: RwLock<Option<Arc<Mutex<AgentMailbox>>>> = RwLock::new(None);

pub struct MailboxRegistry;

impl MailboxRegistry {
    pub fn init() -> Arc<Mutex<AgentMailbox>> {
        let mailbox = Arc::new(Mutex::new(AgentMailbox::new()));
        if let Ok(mut store) = MAILBOX_STORE.write() {
            *store = Some(mailbox.clone());
        }
        mailbox
    }

    pub fn get() -> Arc<Mutex<AgentMailbox>> {
        MAILBOX_STORE
            .read()
            .ok()
            .and_then(|guard| guard.clone())
            .unwrap_or_else(|| Self::init())
    }
}

pub struct MailboxTool;

impl MailboxTool {
    pub fn send(to: impl Into<String>, from: impl Into<String>, payload: Value) -> AgentMessage {
        let message = AgentMessage::new(from, to, payload);
        match MailboxRegistry::get().lock() {
            Ok(mut mb) => mb.send(message.clone()),
            Err(_) => {}
        }
        message
    }

    pub fn receive(agent: &str, unread_only: bool) -> Vec<AgentMessage> {
        match MailboxRegistry::get().lock() {
            Ok(mb) => mb.receive(agent, unread_only).into_iter().cloned().collect(),
            Err(_) => Vec::new(),
        }
    }

    pub fn mark_read(agent: &str, ids: &[String]) {
        match MailboxRegistry::get().lock() {
            Ok(mut mb) => mb.mark_read(agent, ids),
            Err(_) => {}
        }
    }

    pub fn sent_by(agent: &str) -> Vec<AgentMessage> {
        match MailboxRegistry::get().lock() {
            Ok(mb) => mb.sent_by(agent).into_iter().cloned().collect(),
            Err(_) => Vec::new(),
        }
    }
}
