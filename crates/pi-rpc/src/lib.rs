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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RpcRequest {
    pub jsonrpc: Option<String>,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RpcNotification {
    pub jsonrpc: String,
    pub method: String,
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

    /// Parses and validates a raw input line according to JSON-RPC 2.0.
    /// Returns `Ok(RpcRequest)` for valid requests or notifications.
    /// Returns `Err(Box<RpcResponse>)` with code -32700 for JSON syntax errors,
    /// or code -32600 for JSON-RPC structural/version violations.
    pub fn parse_raw_request(raw: &str) -> Result<RpcRequest, Box<RpcResponse>> {
        let val: Value = match serde_json::from_str(raw) {
            Ok(v) => v,
            Err(_) => {
                return Err(Box::new(RpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Value::Null,
                    result: None,
                    error: Some(RpcError {
                        code: -32700,
                        message: "Parse error: Invalid JSON was received by the server."
                            .to_string(),
                        data: None,
                    }),
                }));
            }
        };

        let obj = match val.as_object() {
            Some(map) => map,
            None => {
                return Err(Box::new(RpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Value::Null,
                    result: None,
                    error: Some(RpcError {
                        code: -32600,
                        message: "Invalid Request: Top-level payload must be a JSON object."
                            .to_string(),
                        data: None,
                    }),
                }));
            }
        };

        let id = obj.get("id").cloned();
        let fallback_id = id.clone().unwrap_or(Value::Null);

        if let Some(ver_val) = obj.get("jsonrpc")
            && ver_val.as_str() != Some("2.0")
        {
            return Err(Box::new(RpcResponse {
                jsonrpc: "2.0".to_string(),
                id: fallback_id,
                result: None,
                error: Some(RpcError {
                    code: -32600,
                    message: "Invalid Request: 'jsonrpc' version must be '2.0'.".to_string(),
                    data: None,
                }),
            }));
        }

        let method = match obj.get("method").and_then(|m| m.as_str()) {
            Some(m) if !m.is_empty() => m.to_string(),
            _ => {
                return Err(Box::new(RpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: fallback_id,
                    result: None,
                    error: Some(RpcError {
                        code: -32600,
                        message: "Invalid Request: 'method' must be a non-empty string."
                            .to_string(),
                        data: None,
                    }),
                }));
            }
        };

        let params = obj.get("params").cloned().unwrap_or(Value::Null);

        Ok(RpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id,
            method,
            params,
        })
    }

    pub async fn handle_request<F>(&self, req: RpcRequest, mut notify_tx: F) -> RpcResponse
    where
        F: FnMut(RpcNotification) + Send + 'static,
    {
        let id = req.id.unwrap_or(Value::Null);

        match req.method.as_str() {
            "pi/ping" => RpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(serde_json::json!({
                    "status": "ok",
                    "version": env!("CARGO_PKG_VERSION")
                })),
                error: None,
            },
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
                let force_refresh = req
                    .params
                    .get("force")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
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
                let name = req
                    .params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if name.trim().is_empty() {
                    return RpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: None,
                        error: Some(RpcError {
                            code: -32602,
                            message: "Invalid params: 'name' cannot be empty".to_string(),
                            data: None,
                        }),
                    };
                }
                let args = req
                    .params
                    .get("arguments")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));
                let call_id = req
                    .params
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("rpc_call")
                    .to_string();

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
                    result: Some(
                        serde_json::json!({ "servers": servers, "tool_count": mgr.cached_tools.len() }),
                    ),
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
                        "active_node_id": guard.session_tree.active_node_id,
                        "history": history
                    })),
                    error: None,
                }
            }
            "pi/session/rewind" => {
                let target_id = req
                    .params
                    .get("node_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if target_id.trim().is_empty() {
                    return RpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: None,
                        error: Some(RpcError {
                            code: -32602,
                            message: "Invalid params: 'node_id' cannot be empty".to_string(),
                            data: None,
                        }),
                    };
                }
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
                let branch_node_id = guard
                    .session_tree
                    .append_child(pi_session::Role::System, "Forked branch point".to_string());
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
                let node_a = req
                    .params
                    .get("node_a")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let node_b = req
                    .params
                    .get("node_b")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
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
                if target_id.trim().is_empty() {
                    return RpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: None,
                        error: Some(RpcError {
                            code: -32602,
                            message:
                                "Invalid params: 'target_node_id' or 'node_id' cannot be empty"
                                    .to_string(),
                            data: None,
                        }),
                    };
                }
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
                let prompt = req
                    .params
                    .get("prompt")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
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
                        TurnEvent::ContextPrepared { token_estimate } => {
                            notify_tx(RpcNotification {
                                jsonrpc: "2.0".to_string(),
                                method: "pi/contextPrepared".to_string(),
                                params: serde_json::json!({ "token_estimate": token_estimate }),
                            });
                        }
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
                        TurnEvent::ContextCompacted {
                            old_turns,
                            new_summary_len,
                        } => {
                            notify_tx(RpcNotification {
                                jsonrpc: "2.0".to_string(),
                                method: "pi/contextCompacted".to_string(),
                                params: serde_json::json!({
                                    "old_turns": old_turns,
                                    "new_summary_len": new_summary_len
                                }),
                            });
                        }
                        TurnEvent::TurnCompleted { total_tokens } => {
                            notify_tx(RpcNotification {
                                jsonrpc: "2.0".to_string(),
                                method: "pi/turnCompleted".to_string(),
                                params: serde_json::json!({
                                    "total_tokens": total_tokens
                                }),
                            });
                        }
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

    /// Runs the JSON-RPC event loop over generic AsyncRead and AsyncWrite streams.
    /// Ensures strictly ordered FIFO notification streaming and clean flushing before responses.
    pub async fn run_reader_writer_loop<R, W>(
        server: Arc<RpcServer>,
        reader: R,
        writer: W,
    ) -> Result<()>
    where
        R: tokio::io::AsyncRead + Unpin + Send + 'static,
        W: tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        let stdout_lock = Arc::new(tokio::sync::Mutex::new(writer));
        let mut buf_reader = BufReader::new(reader);
        let mut line = String::new();

        while buf_reader.read_line(&mut line).await? > 0 {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                match Self::parse_raw_request(trimmed) {
                    Ok(req) => {
                        let is_notification = req.id.is_none();
                        let server_clone = Arc::clone(&server);
                        let stdout_clone = Arc::clone(&stdout_lock);

                        let (tx, mut rx) =
                            tokio::sync::mpsc::unbounded_channel::<RpcNotification>();

                        let writer_handle = tokio::spawn(async move {
                            let mut out = stdout_clone.lock().await;
                            while let Some(notif) = rx.recv().await {
                                if let Ok(notif_json) = serde_json::to_string(&notif) {
                                    let _ =
                                        out.write_all(format!("{}\n", notif_json).as_bytes()).await;
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

                        if !is_notification && let Ok(resp_json) = serde_json::to_string(&resp) {
                            let mut out = stdout_lock.lock().await;
                            let _ = out.write_all(format!("{}\n", resp_json).as_bytes()).await;
                            let _ = out.flush().await;
                        }
                    }
                    Err(err_resp) => {
                        if let Ok(resp_json) = serde_json::to_string(&err_resp) {
                            let mut out = stdout_lock.lock().await;
                            let _ = out.write_all(format!("{}\n", resp_json).as_bytes()).await;
                            let _ = out.flush().await;
                        }
                    }
                }
            }
            line.clear();
        }

        Ok(())
    }

    /// Runs the JSON-RPC daemon over stdin/stdout.
    pub async fn run_stdin_stdout_loop(initial_model: Option<&str>) -> Result<()> {
        let server = Arc::new(if let Some(m) = initial_model {
            Self::new(m)
        } else {
            Self::default()
        });
        Self::run_reader_writer_loop(server, tokio::io::stdin(), tokio::io::stdout()).await
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

        // Empty tool name error check
        let req_empty = RpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(serde_json::json!(31)),
            method: "pi/tools/execute".to_string(),
            params: serde_json::json!({ "name": "" }),
        };
        let resp_empty = server.handle_request(req_empty, |_| {}).await;
        assert_eq!(resp_empty.error.unwrap().code, -32602);
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
    async fn test_rpc_session_lifecycle() {
        let server = RpcServer::new("kilo/deepseek-r1");

        // 1. Session new
        let req_new = RpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(serde_json::json!(50)),
            method: "pi/session/new".to_string(),
            params: serde_json::json!({}),
        };
        let resp_new = server.handle_request(req_new, |_| {}).await;
        assert_eq!(resp_new.id, serde_json::json!(50));
        let res_new = resp_new.result.expect("session new result");
        assert!(res_new.get("session_id").is_some());

        // 2. Session fork
        let req_fork = RpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(serde_json::json!(51)),
            method: "pi/session/fork".to_string(),
            params: serde_json::json!({}),
        };
        let resp_fork = server.handle_request(req_fork, |_| {}).await;
        let res_fork = resp_fork.result.expect("session fork result");
        let branch_id = res_fork
            .get("branch_node_id")
            .and_then(|v| v.as_str())
            .unwrap();

        // 3. Session history
        let req_hist = RpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(serde_json::json!(52)),
            method: "pi/session/history".to_string(),
            params: serde_json::json!({}),
        };
        let resp_hist = server.handle_request(req_hist, |_| {}).await;
        let hist_res = resp_hist.result.expect("history result");
        assert!(hist_res.get("history").is_some());

        // 4. Session rewind
        let req_rewind = RpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(serde_json::json!(53)),
            method: "pi/session/rewind".to_string(),
            params: serde_json::json!({ "node_id": branch_id }),
        };
        let resp_rewind = server.handle_request(req_rewind, |_| {}).await;
        let rewind_res = resp_rewind.result.expect("rewind result");
        assert_eq!(rewind_res.get("success").unwrap(), true);

        // Empty rewind param
        let req_empty_rewind = RpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(serde_json::json!(54)),
            method: "pi/session/rewind".to_string(),
            params: serde_json::json!({ "node_id": "" }),
        };
        let resp_empty_rewind = server.handle_request(req_empty_rewind, |_| {}).await;
        assert_eq!(resp_empty_rewind.error.unwrap().code, -32602);
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

        // Model set empty error check
        let req_set_empty = RpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(serde_json::json!(12)),
            method: "pi/model/set".to_string(),
            params: serde_json::json!({ "model": "" }),
        };
        let resp_set_empty = server.handle_request(req_set_empty, |_| {}).await;
        assert_eq!(resp_set_empty.error.unwrap().code, -32602);
    }

    #[tokio::test]
    async fn test_rpc_skills_and_mcp_list() {
        let server = RpcServer::new("kilo/deepseek-r1");

        let req_skills = RpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(serde_json::json!(13)),
            method: "pi/skills/list".to_string(),
            params: serde_json::json!({}),
        };
        let resp_skills = server.handle_request(req_skills, |_| {}).await;
        assert_eq!(resp_skills.id, serde_json::json!(13));
        assert!(resp_skills.result.unwrap().get("skills").is_some());

        let req_mcp = RpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(serde_json::json!(14)),
            method: "pi/mcp/list".to_string(),
            params: serde_json::json!({}),
        };
        let resp_mcp = server.handle_request(req_mcp, |_| {}).await;
        assert_eq!(resp_mcp.id, serde_json::json!(14));
        assert!(resp_mcp.result.unwrap().get("servers").is_some());
    }

    #[test]
    fn test_parse_raw_request_protocol_compliance() {
        // 1. Valid request with integer id
        let valid_req = r#"{"jsonrpc": "2.0", "id": 1, "method": "pi/ping", "params": {}}"#;
        let parsed = RpcServer::parse_raw_request(valid_req).expect("should parse valid request");
        assert_eq!(parsed.id, Some(serde_json::json!(1)));
        assert_eq!(parsed.method, "pi/ping");

        // 2. Valid request with string id
        let valid_str_id = r#"{"jsonrpc": "2.0", "id": "req-99", "method": "pi/tools/list"}"#;
        let parsed_str =
            RpcServer::parse_raw_request(valid_str_id).expect("should parse valid request");
        assert_eq!(parsed_str.id, Some(serde_json::json!("req-99")));

        // 3. Valid notification (no id)
        let valid_notif = r#"{"jsonrpc": "2.0", "method": "pi/ping"}"#;
        let parsed_notif =
            RpcServer::parse_raw_request(valid_notif).expect("should parse valid notification");
        assert!(parsed_notif.id.is_none());

        // 4. Parse error -32700 for malformed JSON
        let malformed = r#"{"jsonrpc": "2.0", "method": "pi/ping", bad json}"#;
        let err_parse = RpcServer::parse_raw_request(malformed).unwrap_err();
        assert_eq!(err_parse.error.unwrap().code, -32700);
        assert_eq!(err_parse.id, Value::Null);

        // 5. Invalid Request -32600 for non-object JSON (array, primitive)
        let array_payload = r#"[1, 2, 3]"#;
        let err_array = RpcServer::parse_raw_request(array_payload).unwrap_err();
        assert_eq!(err_array.error.unwrap().code, -32600);

        let primitive_payload = r#""plain string""#;
        let err_prim = RpcServer::parse_raw_request(primitive_payload).unwrap_err();
        assert_eq!(err_prim.error.unwrap().code, -32600);

        // 6. Invalid Request -32600 for missing method
        let missing_method = r#"{"jsonrpc": "2.0", "id": 100}"#;
        let err_missing = RpcServer::parse_raw_request(missing_method).unwrap_err();
        assert_eq!(err_missing.error.unwrap().code, -32600);
        assert_eq!(err_missing.id, serde_json::json!(100));

        // 7. Invalid Request -32600 for invalid jsonrpc version
        let invalid_ver = r#"{"jsonrpc": "1.0", "id": 101, "method": "pi/ping"}"#;
        let err_ver = RpcServer::parse_raw_request(invalid_ver).unwrap_err();
        assert_eq!(err_ver.error.unwrap().code, -32600);
        assert_eq!(err_ver.id, serde_json::json!(101));
    }

    #[tokio::test]
    async fn test_run_reader_writer_loop_e2e_streaming_and_suppression() {
        let server = Arc::new(RpcServer::new("openai/gpt-4o"));

        // Simulate incoming JSON-RPC stream with:
        // 1. Valid ping request (id: 1)
        // 2. Notification (id: none) -> Must NOT output a response frame
        // 3. Malformed JSON -> -32700
        // 4. Invalid request -> -32600
        // 5. Model get request (id: 5) -> verify initial model "gpt-4o"
        let input_data = [
            r#"{"jsonrpc": "2.0", "id": 1, "method": "pi/ping"}"#,
            r#"{"jsonrpc": "2.0", "method": "pi/ping"}"#,
            r#"{"jsonrpc": "2.0", invalid_json"#,
            r#"{"jsonrpc": "1.0", "id": 4, "method": "pi/ping"}"#,
            r#"{"jsonrpc": "2.0", "id": 5, "method": "pi/model/get"}"#,
        ]
        .join("\n")
            + "\n";

        let (mut client_tx, server_rx) = tokio::io::duplex(65536);
        let (server_tx, client_rx) = tokio::io::duplex(65536);

        let server_handle = tokio::spawn(async move {
            RpcServer::run_reader_writer_loop(server, server_rx, server_tx).await
        });

        // Send all input lines to the server
        client_tx.write_all(input_data.as_bytes()).await.unwrap();
        client_tx.flush().await.unwrap();
        drop(client_tx); // Signal EOF to server reader

        let mut output_lines = Vec::new();
        let mut buf_reader = BufReader::new(client_rx);
        let mut line = String::new();
        while buf_reader.read_line(&mut line).await.unwrap() > 0 {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                output_lines.push(trimmed.to_string());
            }
            line.clear();
        }

        server_handle
            .await
            .unwrap()
            .expect("server loop should exit cleanly on EOF");

        // Exactly 4 response lines expected (the notification was suppressed!)
        assert_eq!(
            output_lines.len(),
            4,
            "Expected 4 output lines, got: {:?}",
            output_lines
        );

        // Line 1: Ping response (id: 1)
        let resp1: Value = serde_json::from_str(&output_lines[0]).unwrap();
        assert_eq!(resp1.get("id").unwrap(), 1);
        assert_eq!(resp1.get("result").unwrap().get("status").unwrap(), "ok");

        // Line 2: Parse error (id: null, code: -32700)
        let resp2: Value = serde_json::from_str(&output_lines[1]).unwrap();
        assert_eq!(resp2.get("id").unwrap(), &Value::Null);
        assert_eq!(resp2.get("error").unwrap().get("code").unwrap(), -32700);

        // Line 3: Invalid request error (id: 4, code: -32600)
        let resp3: Value = serde_json::from_str(&output_lines[2]).unwrap();
        assert_eq!(resp3.get("id").unwrap(), 4);
        assert_eq!(resp3.get("error").unwrap().get("code").unwrap(), -32600);

        // Line 4: Model get response (id: 5, model: gpt-4o, provider: openai)
        let resp4: Value = serde_json::from_str(&output_lines[3]).unwrap();
        assert_eq!(resp4.get("id").unwrap(), 5);
        assert_eq!(resp4.get("result").unwrap().get("model").unwrap(), "gpt-4o");
        assert_eq!(
            resp4.get("result").unwrap().get("provider").unwrap(),
            "openai"
        );
    }

    #[tokio::test]
    async fn test_rpc_empty_prompt_validation() {
        let server = RpcServer::new("kilo/deepseek-r1");
        let req = RpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(serde_json::json!(88)),
            method: "pi/prompt".to_string(),
            params: serde_json::json!({ "prompt": "   " }),
        };
        let resp = server.handle_request(req, |_| {}).await;
        assert_eq!(resp.id, serde_json::json!(88));
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32602);
    }

    #[tokio::test]
    async fn test_mpsc_ordered_fifo_notification_flushing() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<RpcNotification>();

        let consumer = tokio::spawn(async move {
            let mut out = Vec::new();
            while let Some(notif) = rx.recv().await {
                out.push(notif);
            }
            out
        });

        for i in 0..20 {
            tx.send(RpcNotification {
                jsonrpc: "2.0".to_string(),
                method: "pi/streamingChunk".to_string(),
                params: serde_json::json!({ "chunk_index": i }),
            })
            .unwrap();
        }

        drop(tx);
        let delivered = consumer.await.unwrap();
        assert_eq!(delivered.len(), 20);
        for (i, notif) in delivered.iter().enumerate() {
            assert_eq!(notif.params.get("chunk_index").unwrap(), i);
        }
    }
}
