use pi_core::{AlfredAdvisory, SpecialistIdentity, SpecialistInfo};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonRequest {
    #[serde(default)]
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonResponse {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<DaemonError>,
}

impl DaemonResponse {
    pub fn ok(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<serde_json::Value>, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(DaemonError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonStatusInfo {
    pub uptime_secs: u64,
    pub memory_count: usize,
    pub reversible_actions: usize,
    pub active_specialist: SpecialistIdentity,
    pub specialists: Vec<SpecialistInfo>,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonTurnParams {
    pub prompt: String,
    #[serde(default)]
    pub specialist: Option<SpecialistIdentity>,
    #[serde(default)]
    pub workspace_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonTurnResult {
    pub specialist: SpecialistIdentity,
    pub output_text: String,
    pub advisory: Option<AlfredAdvisory>,
    pub execution_plan: Option<String>,
}
