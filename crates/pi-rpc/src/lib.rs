use anyhow::Result;
use pi_core::{AgentLoop, SkillRegistry, TurnEvent};
use pi_providers::{ModelCatalog, ModelConfig};
use pi_session::SessionTree;
use pi_tools::{ToolCall, ToolExecutor};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    pub jsonrpc: Option<String>,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcNotification {
    pub jsonrpc: String,
    pub method: String,
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

pub struct RpcServer {
    pub agent_loop: Arc<Mutex<AgentLoop>>,
}

impl Default for RpcServer {
    fn default() -> Self {
        Self::new("kilo/deepseek-r1")
    }
}

impl RpcServer {
    pub fn new(model_id: &str) -> Self {
        let model_cfg = ModelConfig::resolve(model_id);
        let agent_loop = AgentLoop::new(model_cfg);

        Self {
            agent_loop: Arc::new(Mutex::new(agent_loop)),
        }
    }

    pub async fn handle_request<F>(&self, req: RpcRequest, mut notify_tx: F) -> RpcResponse
    where
        F: FnMut(RpcNotification) + Send + 'static,
    {
        let id = req.id.unwrap_or(Value::Null);

        match req.method.as_str() {
            "pi/ping" => {
                RpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(serde_json::json!({
                        "status": "ok",
                        "version": env!("CARGO_PKG_VERSION")
                    })),
                    error: None,
                }
            }
            "pi/model/get" => {
                let guard = self.agent_loop.lock().await;
                RpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(serde_json::json!({
                        "model": guard.model_config.model_id,
                        "provider": guard.model_config.provider,
                        "base_url": guard.model_config.base_url
                    })),
                    error: None,
                }
            }
            "pi/model/set" => {
                let model_id = req
                    .params
                    .get("model")
                    .or_else(|| req.params.get("model_id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if model_id.trim().is_empty() {
                    return RpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: None,
                        error: Some(RpcError {
                            code: -32602,
                            message: "Invalid params: 'model' cannot be empty".to_string(),
                            data: None,
                        }),
                    };
                }
                let model_cfg = ModelConfig::resolve(model_id);
                let mut guard = self.agent_loop.lock().await;
                guard.model_config = model_cfg.clone();
                RpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(serde_json::json!({
                        "success": true,
                        "model": guard.model_config.model_id,
                        "provider": guard.model_config.provider
                    })),
                    error: None,
                }
            }
            "pi/models" => {
                let force_refresh = req.params.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
                let models = ModelCatalog::get_models(force_refresh).await;
                RpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(serde_json::json!({ "models": models })),
                    error: None,
                }
            }
            "pi/tools/list" => {
                let defs = ToolExecutor::tool_definitions();
                RpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(serde_json::json!({ "tools": defs })),
                    error: None,
                }
            }
            "pi/tools/execute" => {
                let name = req.params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let args = req.params.get("arguments").cloned().unwrap_or(serde_json::json!({}));
                let call_id = req.params.get("id").and_then(|v| v.as_str()).unwrap_or("rpc_call").to_string();

                let call = ToolCall {
                    id: call_id,
                    name: name.to_string(),
                    arguments: args,
                };
                let tool_res = ToolExecutor::execute(&call).await;

                RpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(serde_json::json!({
                        "output": tool_res.output,
                        "is_error": tool_res.is_error
                    })),
                    error: None,
                }
            }
            "pi/skills/list" => {
                let registry = SkillRegistry::new();
                let skills: Vec<serde_json::Value> = registry
                    .skills
                    .into_iter()
                    .map(|s| {
                        serde_json::json!({
                            "name": s.name,
                            "description": s.description,
                            "path": s.path.to_string_lossy()
                        })
                    })
                    .collect();

                RpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(serde_json::json!({ "skills": skills })),
                    error: None,
                }
            }
            "pi/mcp/list" => {
                let mcp_mgr = pi_tools::get_mcp_manager();
                let mgr = mcp_mgr.lock().await;
                let servers: Vec<serde_json::Value> = mgr
                    .servers
                    .values()
                    .map(|s| {
                        serde_json::json!({
                            "name": s.name,
                            "source_agent": s.source_agent,
                            "transport": format!("{:?}", s.transport),
                            "command": s.command,
                            "args": s.args,
                            "url": s.url,
                            "disabled": s.disabled
                        })
                    })
                    .collect();

                RpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(serde_json::json!({ "servers": servers, "tool_count": mgr.cached_tools.len() })),
                    error: None,
                }
            }
            "pi/session/history" => {
                let guard = self.agent_loop.lock().await;
                let history = guard.session_tree.get_active_branch_history();
                RpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(serde_json::json!({
                        "session_id": guard.session_tree.session_id,
                        "history": history
                    })),
                    error: None,
                }
            }
            "pi/session/rewind" => {
                let target_id = req.params.get("node_id").and_then(|v| v.as_str()).unwrap_or("");
                let mut guard = self.agent_loop.lock().await;
                let success = guard.session_tree.rewind_to(target_id);

                RpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(serde_json::json!({
                        "success": success,
                        "active_node_id": guard.session_tree.active_node_id
                    })),
                    error: None,
                }
            }
            "pi/session/fork" => {
                let mut guard = self.agent_loop.lock().await;
                let branch_node_id = guard.session_tree.append_child(pi_session::Role::System, "Forked branch point".to_string());
                RpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(serde_json::json!({
                        "branch_node_id": branch_node_id,
                        "session_id": guard.session_tree.session_id,
                        "active_node_id": guard.session_tree.active_node_id
                    })),
                    error: None,
                }
            }
            "pi/session/new" => {
                let mut guard = self.agent_loop.lock().await;
                guard.session_tree = SessionTree::new();
                RpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(serde_json::json!({
                        "session_id": guard.session_tree.session_id,
                        "root_id": guard.session_tree.root_id
                    })),
                    error: None,
                }
            }
            "pi/session/trajectory" => {
                let branch_node_id = req
                    .params
                    .get("branch_node_id")
                    .or_else(|| req.params.get("node_id"))
                    .and_then(|v| v.as_str());
                let guard = self.agent_loop.lock().await;
                let trajectory = guard.session_tree.export_trajectory(branch_node_id);
                RpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(serde_json::json!({
                        "trajectory": trajectory
                    })),
                    error: None,
                }
            }
            "pi/session/diff" => {
                let node_a = req.params.get("node_a").and_then(|v| v.as_str()).unwrap_or("");
                let node_b = req.params.get("node_b").and_then(|v| v.as_str()).unwrap_or("");
                let guard = self.agent_loop.lock().await;
                let diff = guard.session_tree.diff_branches(node_a, node_b);
                RpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(serde_json::json!({
                        "diff": diff
                    })),
                    error: None,
                }
            }
            "pi/session/simulate_rewind" => {
                let target_id = req
                    .params
                    .get("target_node_id")
                    .or_else(|| req.params.get("node_id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let guard = self.agent_loop.lock().await;
                let sim_history = guard.session_tree.simulate_rewind_to(target_id);
                RpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(serde_json::json!({
                        "simulated_history": sim_history,
                        "target_node_id": target_id
                    })),
                    error: None,
                }
            }
            "pi/prompt" => {
                let prompt = req.params.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
                if prompt.trim().is_empty() {
                    return RpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: None,
                        error: Some(RpcError {
                            code: -32602,
                            message: "Invalid params: 'prompt' cannot be empty".to_string(),
                            data: None,
                        }),
                    };
                }

                let mut guard = self.agent_loop.lock().await;
                let result = guard
                    .run_turn(prompt, |evt| match evt {
                        TurnEvent::ModelStreaming { chunk } => {
                            notify_tx(RpcNotification {
                                jsonrpc: "2.0".to_string(),
                                method: "pi/streamingChunk".to_string(),
                                params: serde_json::json!({ "chunk": chunk }),
                            });
                        }
                        TurnEvent::ToolExecuting {
                            tool_name,
                            tool_call_id,
                        } => {
                            notify_tx(RpcNotification {
                                jsonrpc: "2.0".to_string(),
                                method: "pi/toolExecuting".to_string(),
                                params: serde_json::json!({
                                    "tool_name": tool_name,
                                    "tool_call_id": tool_call_id
                                }),
                            });
                        }
                        TurnEvent::ToolCompleted {
                            tool_name,
                            is_error,
                        } => {
                            notify_tx(RpcNotification {
                                jsonrpc: "2.0".to_string(),
                                method: "pi/toolCompleted".to_string(),
                                params: serde_json::json!({
                                    "tool_name": tool_name,
                                    "is_error": is_error
                                }),
                            });
                        }
                        _ => {}
                    })
                    .await;

                match result {
                    Ok(response) => RpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: Some(serde_json::json!({ "response": response })),
                        error: None,
                    },
                    Err(err) => RpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: None,
                        error: Some(RpcError {
                            code: -32603,
                            message: format!("Agent turn failed: {}", err),
                            data: None,
                        }),
                    },
                }
            }
            _ => RpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(RpcError {
                    code: -32601,
                    message: format!("Method not found: {}", req.method),
                    data: None,
                }),
            },
        }
    }

    pub async fn run_stdin_stdout_loop() -> Result<()> {
        let server = Arc::new(Self::default());
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        let stdout_lock = Arc::new(tokio::sync::Mutex::new(stdout));
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();

        while reader.read_line(&mut line).await? > 0 {
            let trimmed = line.trim();
            if !trimmed.is_empty()
                && let Ok(req) = serde_json::from_str::<RpcRequest>(trimmed)
            {
                let server_clone = Arc::clone(&server);
                let stdout_clone = Arc::clone(&stdout_lock);

                let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<RpcNotification>();

                let writer_handle = tokio::spawn(async move {
                    let mut out = stdout_clone.lock().await;
                    while let Some(notif) = rx.recv().await {
                        if let Ok(notif_json) = serde_json::to_string(&notif) {
                            let _ = out.write_all(format!("{}\n", notif_json).as_bytes()).await;
                            let _ = out.flush().await;
                        }
                    }
                });

                let resp = server_clone
                    .handle_request(req, move |notif| {
                        let _ = tx.send(notif);
                    })
                    .await;

                // Wait for all queued streaming notifications to be completely flushed
                let _ = writer_handle.await;

                if let Ok(resp_json) = serde_json::to_string(&resp) {
                    let mut out = stdout_lock.lock().await;
                    let _ = out.write_all(format!("{}\n", resp_json).as_bytes()).await;
                    let _ = out.flush().await;
                }
            }
            line.clear();
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rpc_models_and_tools_list() {
        let server = RpcServer::new("kilo/deepseek-r1");

        let req_models = RpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(serde_json::json!(1)),
            method: "pi/models".to_string(),
            params: serde_json::json!({}),
        };
        let resp = server.handle_request(req_models, |_| {}).await;
        assert_eq!(resp.id, serde_json::json!(1));
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());

        let req_tools = RpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(serde_json::json!(2)),
            method: "pi/tools/list".to_string(),
            params: serde_json::json!({}),
        };
        let resp_tools = server.handle_request(req_tools, |_| {}).await;
        assert_eq!(resp_tools.id, serde_json::json!(2));
        assert!(resp_tools.result.is_some());
    }

    #[tokio::test]
    async fn test_rpc_tool_execution() {
        let server = RpcServer::new("kilo/deepseek-r1");

        let req = RpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(serde_json::json!(3)),
            method: "pi/tools/execute".to_string(),
            params: serde_json::json!({
                "name": "git",
                "arguments": { "action": "status" }
            }),
        };
        let resp = server.handle_request(req, |_| {}).await;
        assert_eq!(resp.id, serde_json::json!(3));
        assert!(resp.result.unwrap().get("output").is_some());
    }

    #[tokio::test]
    async fn test_rpc_unknown_method() {
        let server = RpcServer::new("kilo/deepseek-r1");

        let req = RpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(serde_json::json!(4)),
            method: "pi/unknown_endpoint".to_string(),
            params: serde_json::json!({}),
        };
        let resp = server.handle_request(req, |_| {}).await;
        assert_eq!(resp.id, serde_json::json!(4));
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32601);
    }

    #[tokio::test]
    async fn test_rpc_session_fork() {
        let server = RpcServer::new("kilo/deepseek-r1");

        let req = RpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(serde_json::json!(5)),
            method: "pi/session/fork".to_string(),
            params: serde_json::json!({}),
        };
        let resp = server.handle_request(req, |_| {}).await;
        assert_eq!(resp.id, serde_json::json!(5));
        assert!(resp.result.is_some());
        let res_obj = resp.result.unwrap();
        assert!(res_obj.get("branch_node_id").is_some());
    }

    #[tokio::test]
    async fn test_rpc_session_trajectory_and_diff() {
        let server = RpcServer::new("kilo/deepseek-r1");

        // Request trajectory export
        let req_traj = RpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(serde_json::json!(6)),
            method: "pi/session/trajectory".to_string(),
            params: serde_json::json!({}),
        };
        let resp_traj = server.handle_request(req_traj, |_| {}).await;
        assert_eq!(resp_traj.id, serde_json::json!(6));
        let traj_res = resp_traj.result.expect("result should exist");
        assert!(traj_res.get("trajectory").is_some());

        // Request branch diff
        let req_diff = RpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(serde_json::json!(7)),
            method: "pi/session/diff".to_string(),
            params: serde_json::json!({
                "node_a": "node1",
                "node_b": "node2"
            }),
        };
        let resp_diff = server.handle_request(req_diff, |_| {}).await;
        assert_eq!(resp_diff.id, serde_json::json!(7));
        let diff_res = resp_diff.result.expect("result should exist");
        assert!(diff_res.get("diff").is_some());

        // Request simulate rewind
        let req_sim = RpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(serde_json::json!(8)),
            method: "pi/session/simulate_rewind".to_string(),
            params: serde_json::json!({
                "target_node_id": "non_existent"
            }),
        };
        let resp_sim = server.handle_request(req_sim, |_| {}).await;
        assert_eq!(resp_sim.id, serde_json::json!(8));
        let sim_res = resp_sim.result.expect("result should exist");
        assert!(sim_res.get("simulated_history").is_some());
    }

    #[tokio::test]
    async fn test_rpc_ping_and_model_methods() {
        let server = RpcServer::new("kilo/deepseek-r1");

        // Ping test
        let req_ping = RpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(serde_json::json!(9)),
            method: "pi/ping".to_string(),
            params: serde_json::json!({}),
        };
        let resp_ping = server.handle_request(req_ping, |_| {}).await;
        assert_eq!(resp_ping.id, serde_json::json!(9));
        let ping_res = resp_ping.result.expect("ping result");
        assert_eq!(ping_res.get("status").unwrap(), "ok");

        // Model get
        let req_get = RpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(serde_json::json!(10)),
            method: "pi/model/get".to_string(),
            params: serde_json::json!({}),
        };
        let resp_get = server.handle_request(req_get, |_| {}).await;
        assert_eq!(resp_get.id, serde_json::json!(10));
        let get_res = resp_get.result.expect("model get result");
        assert_eq!(get_res.get("model").unwrap(), "deepseek-r1");
        assert_eq!(get_res.get("provider").unwrap(), "kilo");

        // Model set
        let req_set = RpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(serde_json::json!(11)),
            method: "pi/model/set".to_string(),
            params: serde_json::json!({ "model": "anthropic/claude-3-7-sonnet-latest" }),
        };
        let resp_set = server.handle_request(req_set, |_| {}).await;
        assert_eq!(resp_set.id, serde_json::json!(11));
        let set_res = resp_set.result.expect("model set result");
        assert_eq!(set_res.get("success").unwrap(), true);
        assert_eq!(set_res.get("model").unwrap(), "claude-3-7-sonnet-latest");
        assert_eq!(set_res.get("provider").unwrap(), "anthropic");
    }
}
