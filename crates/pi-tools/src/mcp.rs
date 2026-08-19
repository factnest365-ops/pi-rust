use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;

static MCP_MANAGER: OnceLock<Arc<Mutex<McpManager>>> = OnceLock::new();

pub fn get_mcp_manager() -> &'static Arc<Mutex<McpManager>> {
    MCP_MANAGER.get_or_init(|| {
        let mut manager = McpManager::new();
        manager.discover_servers();
        Arc::new(Mutex::new(manager))
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum McpTransportType {
    Stdio,
    Http,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub source_agent: String,
    pub transport: McpTransportType,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub url: Option<String>,
    pub disabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDefinition {
    pub server_name: String,
    pub name: String,
    pub original_name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Default)]
pub struct McpManager {
    pub servers: HashMap<String, McpServerConfig>,
    pub cached_tools: HashMap<String, McpToolDefinition>,
    pub tool_to_server_map: HashMap<String, String>,
}

impl McpManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Auto-discover MCP servers from all known agent and IDE configuration paths
    pub fn discover_servers(&mut self) {
        self.servers.clear();

        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

        // Discovery locations in increasing priority (later ones override earlier ones on name collision)
        let candidate_paths = vec![
            // Hermes, Jarvis, LM Studio, Jan
            ("LMStudio", home.join(".lmstudio").join("mcp.json")),
            ("Jan", home.join("Library").join("Application Support").join("Jan").join("data").join("mcp_config.json")),
            ("Hermes", home.join(".hermes").join("mcp.json")),
            ("Jarvis", home.join(".jarvis").join("mcp.json")),
            // Cursor & Windsurf
            ("Cursor", home.join(".cursor").join("mcp.json")),
            ("Windsurf", home.join(".codeium").join("windsurf").join("mcp_config.json")),
            // Claude Code & Desktop
            ("Claude", home.join(".claude.json")),
            ("Claude", home.join(".claude").join("mcp.json")),
            ("Claude", home.join("Library").join("Application Support").join("Claude").join("claude_desktop_config.json")),
            ("Claude", home.join(".config").join("claude").join("claude_desktop_config.json")),
            // VS Code / Cloud Code
            ("VSCode", home.join("Library").join("Application Support").join("Code").join("User").join("mcp.json")),
            ("VSCode", home.join(".config").join("Code").join("User").join("mcp.json")),
            // Gemini / Antigravity / Cloud Code
            ("Gemini", home.join(".gemini").join("config").join("mcp_config.json")),
            ("Gemini", home.join(".gemini").join("antigravity-cli").join("mcp_config.json")),
            ("Gemini", home.join(".gemini").join("antigravity-ide").join("mcp_config.json")),
            // MCPorter / MC Proctor
            ("MCPorter", home.join(".mcporter").join("mcporter.json")),
            ("MCPorter", home.join(".config").join("mcporter").join("mcporter.json")),
            // Pi Agent Global
            ("PiGlobal", home.join(".pi").join("agent").join("mcp.json")),
            ("PiGlobal", home.join(".pi").join("mcp.json")),
            // Project Local (Highest Priority)
            ("Project", PathBuf::from(".mcp.json")),
            ("Project", PathBuf::from("mcp.json")),
            ("Project", PathBuf::from(".claude").join("mcp.json")),
            ("Project", PathBuf::from(".pi").join("mcp.json")),
        ];

        for (agent_label, path) in candidate_paths {
            if path.exists() && path.is_file() {
                self.load_from_config_file(&path, agent_label);
            }
        }
    }

    fn load_from_config_file(&mut self, path: &Path, source_agent: &str) {
        let Ok(content) = fs::read_to_string(path) else {
            return;
        };
        let Ok(json) = serde_json::from_str::<Value>(&content) else {
            return;
        };

        // Standard format: { "mcpServers": { ... } }
        if let Some(servers_obj) = json.get("mcpServers").and_then(|v| v.as_object()) {
            for (name, cfg) in servers_obj {
                if let Some(parsed) = Self::parse_server_entry(name, cfg, source_agent) {
                    self.servers.insert(name.clone(), parsed);
                }
            }
        }

        // VS Code format: { "servers": { ... } }
        if let Some(servers_obj) = json.get("servers").and_then(|v| v.as_object()) {
            for (name, cfg) in servers_obj {
                if let Some(parsed) = Self::parse_server_entry(name, cfg, source_agent) {
                    self.servers.insert(name.clone(), parsed);
                }
            }
        }
    }

    fn parse_server_entry(name: &str, val: &Value, source_agent: &str) -> Option<McpServerConfig> {
        let command = val.get("command").and_then(|v| v.as_str()).map(ToString::to_string);
        let url = val.get("url").and_then(|v| v.as_str()).map(ToString::to_string);

        if command.is_none() && url.is_none() {
            return None;
        }

        let mut args = Vec::new();
        if let Some(args_arr) = val.get("args").and_then(|v| v.as_array()) {
            for a in args_arr {
                if let Some(s) = a.as_str() {
                    args.push(s.to_string());
                }
            }
        }

        let mut env = HashMap::new();
        if let Some(env_obj) = val.get("env").and_then(|v| v.as_object()) {
            for (k, v) in env_obj {
                if let Some(s) = v.as_str() {
                    env.insert(k.clone(), s.to_string());
                }
            }
        }

        let disabled = val.get("disabled").and_then(|v| v.as_bool()).unwrap_or(false);
        let transport = if url.is_some() {
            McpTransportType::Http
        } else {
            McpTransportType::Stdio
        };

        Some(McpServerConfig {
            name: name.to_string(),
            source_agent: source_agent.to_string(),
            transport,
            command,
            args,
            env,
            url,
            disabled,
        })
    }

    /// Refresh tools from all discovered MCP servers
    pub async fn refresh_all_tools(&mut self) -> Result<()> {
        self.cached_tools.clear();
        self.tool_to_server_map.clear();

        let servers = self.servers.clone();
        for (server_name, config) in servers {
            if config.disabled {
                continue;
            }

            match Self::fetch_server_tools(&config).await {
                Ok(tools) => {
                    for tool in tools {
                        let tool_name = tool.name.clone();
                        self.tool_to_server_map.insert(tool_name.clone(), server_name.clone());
                        self.cached_tools.insert(tool_name, tool);
                    }
                }
                Err(err) => {
                    eprintln!("Failed to fetch tools from MCP server [{}] ({}): {}", server_name, config.source_agent, err);
                }
            }
        }

        Ok(())
    }

    /// Query tools from an individual MCP server via JSON-RPC
    pub async fn fetch_server_tools(config: &McpServerConfig) -> Result<Vec<McpToolDefinition>> {
        match config.transport {
            McpTransportType::Stdio => Self::fetch_stdio_tools(config).await,
            McpTransportType::Http => Self::fetch_http_tools(config).await,
        }
    }

    async fn read_stdio_response<R: tokio::io::AsyncBufRead + Unpin>(
        reader: &mut R,
        expected_id: i64,
        timeout_dur: Duration,
    ) -> Result<Value> {
        let start = std::time::Instant::now();
        let mut line = String::new();

        while start.elapsed() < timeout_dur {
            line.clear();
            let remaining = timeout_dur.saturating_sub(start.elapsed());
            if tokio::time::timeout(remaining, reader.read_line(&mut line)).await?? == 0 {
                break;
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Ok(json_val) = serde_json::from_str::<Value>(trimmed) {
                let id_match = json_val.get("id").map(|id| {
                    id == &serde_json::json!(expected_id)
                        || id.as_str() == Some(&expected_id.to_string())
                        || id.as_i64() == Some(expected_id)
                }).unwrap_or(false);

                if id_match {
                    return Ok(json_val);
                }
            }
        }

        Err(anyhow::anyhow!("Timed out waiting for MCP JSON-RPC response with id {}", expected_id))
    }

    async fn fetch_stdio_tools(config: &McpServerConfig) -> Result<Vec<McpToolDefinition>> {
        let Some(ref cmd) = config.command else {
            return Ok(Vec::new());
        };

        let mut child = Command::new(cmd)
            .args(&config.args)
            .envs(&config.env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let mut stdin = child.stdin.take().ok_or_else(|| anyhow::anyhow!("Failed to open stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow::anyhow!("Failed to open stdout"))?;
        let mut reader = BufReader::new(stdout);

        let fetch_res: Result<Vec<McpToolDefinition>> = async {
            // 1. Send initialize request
            let init_req = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "roots": { "listChanged": true },
                        "sampling": {}
                    },
                    "clientInfo": {
                        "name": "pi-rs",
                        "version": "0.1.0"
                    }
                }
            });
            stdin.write_all(format!("{}\n", serde_json::to_string(&init_req)?).as_bytes()).await?;
            stdin.flush().await?;

            // Read initialize response matching id: 1
            let _init_resp = Self::read_stdio_response(&mut reader, 1, Duration::from_secs(5)).await?;

            // Send notifications/initialized
            let notif = serde_json::json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            });
            stdin.write_all(format!("{}\n", serde_json::to_string(&notif)?).as_bytes()).await?;
            stdin.flush().await?;

            // 2. Send tools/list request
            let list_req = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/list",
                "params": {}
            });
            stdin.write_all(format!("{}\n", serde_json::to_string(&list_req)?).as_bytes()).await?;
            stdin.flush().await?;

            let resp: Value = Self::read_stdio_response(&mut reader, 2, Duration::from_secs(5)).await?;

            let mut tools = Vec::new();
            if let Some(tools_arr) = resp.get("result").and_then(|r| r.get("tools")).and_then(|t| t.as_array()) {
                for t in tools_arr {
                    if let Some(name) = t.get("name").and_then(|n| n.as_str()) {
                        let desc = t.get("description").and_then(|d| d.as_str()).unwrap_or("").to_string();
                        let params = t.get("inputSchema").cloned().unwrap_or(serde_json::json!({
                            "type": "object",
                            "properties": {}
                        }));

                        // Prefix tool name with server name if namespace isolation is needed
                        let namespaced_name = if name.contains('_') || name.contains('-') {
                            name.to_string()
                        } else {
                            format!("{}_{}", config.name, name)
                        };

                        tools.push(McpToolDefinition {
                            server_name: config.name.clone(),
                            name: namespaced_name,
                            original_name: name.to_string(),
                            description: desc,
                            parameters: params,
                        });
                    }
                }
            }

            Ok(tools)
        }.await;

        // Clean up child process unconditionally
        let _ = child.kill().await;
        let _ = child.wait().await;

        fetch_res
    }

    async fn fetch_http_tools(config: &McpServerConfig) -> Result<Vec<McpToolDefinition>> {
        let Some(ref url) = config.url else {
            return Ok(Vec::new());
        };

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()?;

        let list_payload = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {}
        });

        // 2026-07-28 MCP Header-based stateless routing
        let res = client.post(url)
            .header("Mcp-Method", "tools/list")
            .json(&list_payload)
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(anyhow::anyhow!("HTTP error {} from MCP server {}", res.status(), url));
        }

        let resp: Value = res.json().await?;
        let mut tools = Vec::new();

        if let Some(tools_arr) = resp.get("result").and_then(|r| r.get("tools")).and_then(|t| t.as_array()) {
            for t in tools_arr {
                if let Some(name) = t.get("name").and_then(|n| n.as_str()) {
                    let desc = t.get("description").and_then(|d| d.as_str()).unwrap_or("").to_string();
                    let params = t.get("inputSchema").cloned().unwrap_or(serde_json::json!({
                        "type": "object",
                        "properties": {}
                    }));

                    tools.push(McpToolDefinition {
                        server_name: config.name.clone(),
                        name: name.to_string(),
                        original_name: name.to_string(),
                        description: desc,
                        parameters: params,
                    });
                }
            }
        }

        Ok(tools)
    }

    /// Execute an MCP tool via JSON-RPC
    pub async fn execute_tool(&self, tool_name: &str, arguments: &Value) -> Result<String> {
        let Some(server_name) = self.tool_to_server_map.get(tool_name) else {
            return Err(anyhow::anyhow!("MCP tool '{}' not registered to any server", tool_name));
        };

        let Some(config) = self.servers.get(server_name) else {
            return Err(anyhow::anyhow!("MCP server '{}' not found", server_name));
        };

        let original_name = self
            .cached_tools
            .get(tool_name)
            .map(|t| t.original_name.as_str())
            .unwrap_or(tool_name);

        match config.transport {
            McpTransportType::Stdio => Self::execute_stdio_tool(config, original_name, arguments).await,
            McpTransportType::Http => Self::execute_http_tool(config, original_name, arguments).await,
        }
    }

    async fn execute_stdio_tool(config: &McpServerConfig, original_name: &str, arguments: &Value) -> Result<String> {
        let Some(ref cmd) = config.command else {
            return Err(anyhow::anyhow!("No command configured for MCP server {}", config.name));
        };

        let mut child = Command::new(cmd)
            .args(&config.args)
            .envs(&config.env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let mut stdin = child.stdin.take().ok_or_else(|| anyhow::anyhow!("Failed to open stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow::anyhow!("Failed to open stdout"))?;
        let mut reader = BufReader::new(stdout);

        let exec_res: Result<String> = async {
            // 1. Initialize
            let init_req = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": { "name": "pi-rs", "version": "0.1.0" }
                }
            });
            stdin.write_all(format!("{}\n", serde_json::to_string(&init_req)?).as_bytes()).await?;
            stdin.flush().await?;

            let _init_resp = Self::read_stdio_response(&mut reader, 1, Duration::from_secs(10)).await?;

            let notif = serde_json::json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
            stdin.write_all(format!("{}\n", serde_json::to_string(&notif)?).as_bytes()).await?;
            stdin.flush().await?;

            // 2. Call tool
            let call_req = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": {
                    "name": original_name,
                    "arguments": arguments
                }
            });
            stdin.write_all(format!("{}\n", serde_json::to_string(&call_req)?).as_bytes()).await?;
            stdin.flush().await?;

            let resp: Value = Self::read_stdio_response(&mut reader, 2, Duration::from_secs(120)).await?;

            if let Some(err) = resp.get("error") {
                let msg = err.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown MCP error");
                return Err(anyhow::anyhow!("MCP tool error: {}", msg));
            }

            if let Some(result) = resp.get("result") {
                let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
                let mut out = String::new();
                if let Some(content_arr) = result.get("content").and_then(|c| c.as_array()) {
                    for item in content_arr {
                        if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                            out.push_str(text);
                        }
                    }
                }
                if out.is_empty() {
                    out = serde_json::to_string_pretty(result)?;
                }
                if is_error {
                    return Err(anyhow::anyhow!("MCP tool application error: {}", out));
                }
                return Ok(out);
            }

            Ok("MCP tool executed with empty response.".to_string())
        }.await;

        let _ = child.kill().await;
        let _ = child.wait().await;

        exec_res
    }

    async fn execute_http_tool(config: &McpServerConfig, original_name: &str, arguments: &Value) -> Result<String> {
        let Some(ref url) = config.url else {
            return Err(anyhow::anyhow!("No URL configured for HTTP MCP server {}", config.name));
        };

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()?;

        let call_payload = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": original_name,
                "arguments": arguments
            }
        });

        // 2026-07-28 Stateless Header-based routing
        let res = client.post(url)
            .header("Mcp-Method", "tools/call")
            .header("Mcp-Name", original_name)
            .json(&call_payload)
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(anyhow::anyhow!("HTTP error {} from MCP server {}", res.status(), url));
        }

        let resp: Value = res.json().await?;
        if let Some(err) = resp.get("error") {
            let msg = err.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown MCP error");
            return Err(anyhow::anyhow!("MCP tool error: {}", msg));
        }

        if let Some(result) = resp.get("result") {
            let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
            let mut out = String::new();
            if let Some(content_arr) = result.get("content").and_then(|c| c.as_array()) {
                for item in content_arr {
                    if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                        out.push_str(text);
                    }
                }
            }
            if out.is_empty() {
                out = serde_json::to_string_pretty(result)?;
            }
            if is_error {
                return Err(anyhow::anyhow!("MCP tool application error: {}", out));
            }
            return Ok(out);
        }

        Ok("MCP tool executed with empty response.".to_string())
    }

    /// Return all MCP tool definitions formatted as standard JSON Schemas
    pub fn get_tool_definitions(&self) -> Vec<Value> {
        let mut defs = Vec::new();
        for tool in self.cached_tools.values() {
            defs.push(serde_json::json!({
                "name": tool.name,
                "description": format!("[MCP: {}] {}", tool.server_name, tool.description),
                "parameters": tool.parameters
            }));
        }
        defs
    }

    pub fn is_mcp_tool(&self, name: &str) -> bool {
        self.tool_to_server_map.contains_key(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_standard_mcp_config() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("mcp.json");

        let config_json = r#"{
            "mcpServers": {
                "chrome-devtools": {
                    "command": "npx",
                    "args": ["-y", "chrome-devtools-mcp@latest"]
                },
                "remote-db": {
                    "url": "https://mcp.example.com/api"
                }
            }
        }"#;
        fs::write(&config_path, config_json).unwrap();

        let mut manager = McpManager::new();
        manager.load_from_config_file(&config_path, "TestAgent");

        assert_eq!(manager.servers.len(), 2);
        assert!(manager.servers.contains_key("chrome-devtools"));
        assert_eq!(manager.servers["chrome-devtools"].source_agent, "TestAgent");
        assert_eq!(manager.servers["chrome-devtools"].command.as_deref(), Some("npx"));
        assert_eq!(manager.servers["chrome-devtools"].transport, McpTransportType::Stdio);

        assert!(manager.servers.contains_key("remote-db"));
        assert_eq!(manager.servers["remote-db"].transport, McpTransportType::Http);
        assert_eq!(manager.servers["remote-db"].url.as_deref(), Some("https://mcp.example.com/api"));
    }

    #[test]
    fn test_parse_vscode_mcp_config() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("vscode_mcp.json");

        let config_json = r#"{
            "servers": {
                "microsoft/playwright-mcp": {
                    "command": "npx",
                    "args": ["@playwright/mcp@latest"],
                    "type": "stdio"
                }
            }
        }"#;
        fs::write(&config_path, config_json).unwrap();

        let mut manager = McpManager::new();
        manager.load_from_config_file(&config_path, "VSCode");

        assert_eq!(manager.servers.len(), 1);
        assert!(manager.servers.contains_key("microsoft/playwright-mcp"));
        assert_eq!(manager.servers["microsoft/playwright-mcp"].source_agent, "VSCode");
    }

    #[test]
    fn test_auto_discovery_scanner() {
        let mut manager = McpManager::new();
        // Test discovery runs without panic or error
        manager.discover_servers();

        // Also test deterministic discovery from a known mock directory
        let temp_dir = tempfile::tempdir().unwrap();
        let mock_claude = temp_dir.path().join("claude_desktop_config.json");
        let mock_json = r#"{
            "mcpServers": {
                "sqlite": {
                    "command": "uvx",
                    "args": ["mcp-server-sqlite"]
                }
            }
        }"#;
        fs::write(&mock_claude, mock_json).unwrap();
        manager.load_from_config_file(&mock_claude, "Claude Desktop");
        assert!(manager.servers.contains_key("sqlite"));
        assert_eq!(manager.servers["sqlite"].source_agent, "Claude Desktop");
    }

    #[test]
    fn test_mcp_tool_definitions_and_schema() {
        let mut manager = McpManager::new();
        manager.cached_tools.insert(
            "github_create_issue".to_string(),
            McpToolDefinition {
                server_name: "github".to_string(),
                name: "github_create_issue".to_string(),
                original_name: "create_issue".to_string(),
                description: "Create an issue on GitHub".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "title": { "type": "string" },
                        "body": { "type": "string" }
                    },
                    "required": ["title"]
                }),
            },
        );

        let defs = manager.get_tool_definitions();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0]["name"], "github_create_issue");
        assert!(defs[0]["description"].as_str().unwrap().contains("[MCP: github]"));
        assert_eq!(defs[0]["parameters"]["required"][0], "title");
    }

    #[tokio::test]
    async fn test_mcp_http_execution_and_is_error_handling() {
        let mock_server = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = mock_server.local_addr().unwrap();
        let url = format!("http://{}", addr);

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = mock_server.accept().await {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut buf = [0u8; 1024];
                let _ = socket.read(&mut buf).await;
                // Return MCP application error: isError = true
                let body = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": {
                        "isError": true,
                        "content": [
                            { "type": "text", "text": "Database connection refused" }
                        ]
                    }
                }).to_string();
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = socket.write_all(resp.as_bytes()).await;
            }
        });

        let config = McpServerConfig {
            name: "test-db".to_string(),
            source_agent: "test".to_string(),
            transport: McpTransportType::Http,
            command: None,
            args: Vec::new(),
            env: HashMap::new(),
            url: Some(url),
            disabled: false,
        };

        let res = McpManager::execute_http_tool(&config, "query", &serde_json::json!({ "sql": "SELECT 1" })).await;
        assert!(res.is_err());
        let err_msg = res.unwrap_err().to_string();
        assert!(err_msg.contains("MCP tool application error"));
        assert!(err_msg.contains("Database connection refused"));
    }
}
