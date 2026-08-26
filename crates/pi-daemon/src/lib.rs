use anyhow::{Result, anyhow};
use pi_core::{
    AlfredProtocol, FederatedFleet, SpecialistIdentity, StateSynchronizer, TauVault, UndoEngine,
};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;

pub mod cron;
pub use cron::{CronContext, JobsFile};
pub mod ipc;
pub use ipc::{
    DaemonError, DaemonRequest, DaemonResponse, DaemonStatusInfo, DaemonTurnParams,
    DaemonTurnResult,
};

pub struct DaemonServer {
    pub socket_path: PathBuf,
    pub vault: Arc<TauVault>,
    pub fleet: Arc<Mutex<FederatedFleet>>,
    pub undo: Arc<Mutex<UndoEngine>>,
    pub sync: Arc<StateSynchronizer>,
    pub alfred: Arc<Mutex<AlfredProtocol>>,
    pub cron: Arc<CronContext>,
    pub started_at: Instant,
}

impl DaemonServer {
    pub fn new(socket_path: PathBuf, vault: Arc<TauVault>) -> Self {
        let fleet = Arc::new(Mutex::new(FederatedFleet::new(vault.clone())));
        let undo = Arc::new(Mutex::new(UndoEngine::new()));
        let sync_dir = StateSynchronizer::default_dir();
        let sync = Arc::new(StateSynchronizer::new(sync_dir));
        let alfred = Arc::new(Mutex::new(AlfredProtocol::new()));
        let cron = Arc::new(CronContext::default());

        Self {
            socket_path,
            vault,
            fleet,
            undo,
            sync,
            alfred,
            cron,
            started_at: Instant::now(),
        }
    }

    pub fn default_socket_path() -> PathBuf {
        dirs::home_dir()
            .map(|h| h.join(".tau").join("taud.sock"))
            .unwrap_or_else(|| PathBuf::from(".tau/taud.sock"))
    }

    pub fn open_default() -> Result<Self> {
        let vault = Arc::new(TauVault::open_default()?);
        let socket_path = Self::default_socket_path();
        Ok(Self::new(socket_path, vault))
    }

    pub async fn handle_request(&self, req: DaemonRequest) -> DaemonResponse {
        let req_id = req.id.clone();
        match req.method.as_str() {
            "tau/ping" => DaemonResponse::ok(req_id, serde_json::json!("pong")),
            "tau/status" => {
                let fleet = self.fleet.lock().await;
                let undo = self.undo.lock().await;
                let memory_count = self.vault.count_active_memories().unwrap_or(0);
                let jobs = self.cron.load_jobs().unwrap_or_default();
                let info = DaemonStatusInfo {
                    uptime_secs: self.started_at.elapsed().as_secs(),
                    memory_count,
                    reversible_actions: undo.reversible_count(),
                    active_specialist: fleet.active_specialist,
                    specialists: fleet.list_specialists(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    cron_jobs_count: jobs.jobs.len(),
                };
                DaemonResponse::ok(req_id, serde_json::to_value(info).unwrap_or_default())
            }
            "tau/specialists" => {
                let fleet = self.fleet.lock().await;
                let list = fleet.list_specialists();
                DaemonResponse::ok(req_id, serde_json::to_value(list).unwrap_or_default())
            }
            "tau/switchSpecialist" => {
                if let Some(params) = req.params
                    && let Some(id_str) = params.get("specialist").and_then(|s| s.as_str())
                {
                    let spec = match id_str.to_lowercase().as_str() {
                        "friday" => SpecialistIdentity::Friday,
                        "ev" => SpecialistIdentity::Ev,
                        _ => SpecialistIdentity::Jarvis,
                    };
                    let mut fleet = self.fleet.lock().await;
                    fleet.set_active_specialist(spec);
                    return DaemonResponse::ok(
                        req_id,
                        serde_json::json!({ "active_specialist": spec }),
                    );
                }
                DaemonResponse::error(req_id, -32602, "Missing 'specialist' param")
            }
            "tau/alfred" => {
                let (goal, context) = if let Some(params) = &req.params {
                    let g = params.get("goal").and_then(|v| v.as_str()).unwrap_or("");
                    let c = params.get("context").and_then(|v| v.as_str()).unwrap_or("");
                    (g.to_string(), c.to_string())
                } else {
                    (String::new(), String::new())
                };

                let mut alfred = self.alfred.lock().await;
                let advisory = alfred.evaluate_action(&goal, &context);
                DaemonResponse::ok(req_id, serde_json::to_value(advisory).unwrap_or_default())
            }
            "tau/undo" => {
                let mut undo = self.undo.lock().await;
                match undo.undo_last(1) {
                    Ok(messages) => DaemonResponse::ok(
                        req_id,
                        serde_json::json!({ "undone": true, "messages": messages }),
                    ),
                    Err(e) => DaemonResponse::error(req_id, -32000, e.to_string()),
                }
            }
            "tau/sync" => {
                let subject = req
                    .params
                    .as_ref()
                    .and_then(|p| p.get("subject"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("chore: sync cognitive state");

                match self.sync.commit_state_change(subject, None) {
                    Ok(msg) => DaemonResponse::ok(req_id, serde_json::json!({ "status": msg })),
                    Err(e) => DaemonResponse::error(req_id, -32001, e.to_string()),
                }
            }
            "tau/memory/add" => {
                if let Some(params) = req.params {
                    let scope = params
                        .get("scope")
                        .and_then(|s| s.as_str())
                        .unwrap_or("user");
                    let topic = params
                        .get("topic")
                        .and_then(|s| s.as_str())
                        .unwrap_or("general");
                    let content = params.get("content").and_then(|s| s.as_str()).unwrap_or("");
                    match self
                        .vault
                        .record_memory(scope, topic, content, None, None, None)
                    {
                        Ok(id) => {
                            DaemonResponse::ok(req_id, serde_json::json!({ "memory_id": id }))
                        }
                        Err(e) => DaemonResponse::error(req_id, -32002, e.to_string()),
                    }
                } else {
                    DaemonResponse::error(req_id, -32602, "Missing memory params")
                }
            }
            "tau/memory/search" => {
                let query = req
                    .params
                    .as_ref()
                    .and_then(|p| p.get("query"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let limit = req
                    .params
                    .as_ref()
                    .and_then(|p| p.get("limit"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(5) as usize;

                match self.vault.search_hybrid(query, limit) {
                    Ok(entries) => DaemonResponse::ok(
                        req_id,
                        serde_json::to_value(entries).unwrap_or_default(),
                    ),
                    Err(e) => DaemonResponse::error(req_id, -32003, e.to_string()),
                }
            }
            other => DaemonResponse::error(req_id, -32601, format!("Method '{}' not found", other)),
        }
    }

    /// Runs the Unix domain socket server loop until a shutdown signal is received.
    pub async fn run_server(
        self: Arc<Self>,
        mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
    ) -> Result<()> {
        if let Some(parent) = self.socket_path.parent() {
            fs::create_dir_all(parent)?;
        }
        if self.socket_path.exists() {
            let _ = fs::remove_file(&self.socket_path);
        }

        let listener = UnixListener::bind(&self.socket_path)?;

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    break;
                }
                accept_res = listener.accept() => {
                    if let Ok((stream, _)) = accept_res {
                        let server_arc = self.clone();
                        tokio::spawn(async move {
                            let _ = Self::handle_connection(server_arc, stream).await;
                        });
                    }
                }
            }
        }

        // Clean up socket file on graceful shutdown
        if self.socket_path.exists() {
            let _ = fs::remove_file(&self.socket_path);
        }

        Ok(())
    }

    async fn handle_connection(server: Arc<Self>, stream: UnixStream) -> Result<()> {
        let (reader, mut writer) = stream.into_split();
        let mut lines = BufReader::new(reader).lines();

        while let Some(line) = lines.next_line().await? {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let request: DaemonRequest = match serde_json::from_str(trimmed) {
                Ok(r) => r,
                Err(e) => {
                    let err_resp =
                        DaemonResponse::error(None, -32700, format!("Parse error: {}", e));
                    let serialized = serde_json::to_string(&err_resp)?;
                    writer.write_all(serialized.as_bytes()).await?;
                    writer.write_all(b"\n").await?;
                    continue;
                }
            };

            let response = server.handle_request(request).await;
            let serialized = serde_json::to_string(&response)?;
            writer.write_all(serialized.as_bytes()).await?;
            writer.write_all(b"\n").await?;
            writer.flush().await?;
        }

        Ok(())
    }
}

pub struct DaemonClient {
    socket_path: PathBuf,
}

impl DaemonClient {
    pub fn new(socket_path: PathBuf) -> Self {
        Self { socket_path }
    }

    pub fn default_client() -> Self {
        Self::new(DaemonServer::default_socket_path())
    }

    pub fn is_daemon_running(&self) -> bool {
        self.socket_path.exists()
    }

    pub async fn send_request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let stream = UnixStream::connect(&self.socket_path).await?;
        let (reader, mut writer) = stream.into_split();

        let req = DaemonRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: method.to_string(),
            params,
        };

        let serialized = serde_json::to_string(&req)?;
        writer.write_all(serialized.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;

        let mut lines = BufReader::new(reader).lines();
        if let Some(line) = lines.next_line().await? {
            let resp: DaemonResponse = serde_json::from_str(&line)?;
            if let Some(err) = resp.error {
                return Err(anyhow!("Daemon error {}: {}", err.code, err.message));
            }
            if let Some(res) = resp.result {
                return Ok(res);
            }
        }

        Err(anyhow!("Empty response from daemon"))
    }

    pub async fn ping(&self) -> Result<String> {
        let res = self.send_request("tau/ping", None).await?;
        res.as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("Invalid ping response"))
    }

    pub async fn status(&self) -> Result<DaemonStatusInfo> {
        let res = self.send_request("tau/status", None).await?;
        let info: DaemonStatusInfo = serde_json::from_value(res)?;
        Ok(info)
    }

    pub async fn switch_specialist(&self, specialist: SpecialistIdentity) -> Result<()> {
        self.send_request(
            "tau/switchSpecialist",
            Some(serde_json::json!({ "specialist": specialist })),
        )
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_daemon_server_client_e2e_ping_status_and_methods() {
        let tmp = tempdir().unwrap();
        let socket_path = tmp.path().join("taud_test.sock");
        let vault = Arc::new(TauVault::open_in_memory().unwrap());

        let server = Arc::new(DaemonServer::new(socket_path.clone(), vault));
        let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

        let server_handle = {
            let s = server.clone();
            tokio::spawn(async move {
                s.run_server(shutdown_rx).await.unwrap();
            })
        };

        // Give server time to bind
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let client = DaemonClient::new(socket_path.clone());
        assert!(client.is_daemon_running());

        // 1. Ping
        let pong = client.ping().await.unwrap();
        assert_eq!(pong, "pong");

        // 2. Status
        let status = client.status().await.unwrap();
        assert_eq!(status.active_specialist, SpecialistIdentity::Jarvis);
        assert_eq!(status.specialists.len(), 3);

        // 3. Switch Specialist to Friday
        client
            .switch_specialist(SpecialistIdentity::Friday)
            .await
            .unwrap();
        let status_after = client.status().await.unwrap();
        assert_eq!(status_after.active_specialist, SpecialistIdentity::Friday);

        // 4. Memory add and search over daemon IPC
        let add_res = client
            .send_request(
                "tau/memory/add",
                Some(serde_json::json!({
                    "scope": "rule",
                    "topic": "testing",
                    "content": "Verify all IPC endpoints thoroughly"
                })),
            )
            .await
            .unwrap();
        assert!(add_res.get("memory_id").is_some());

        let search_res = client
            .send_request(
                "tau/memory/search",
                Some(serde_json::json!({
                    "query": "thoroughly testing",
                    "limit": 3
                })),
            )
            .await
            .unwrap();
        assert!(!search_res.as_array().unwrap().is_empty());

        // Shutdown
        let _ = shutdown_tx.send(());
        let _ = server_handle.await;

        // Verify socket file was cleaned up
        assert!(!socket_path.exists());
    }
}
