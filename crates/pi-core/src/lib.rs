use anyhow::Result;
use pi_providers::{ChatMessage, ModelConfig, ProviderClient, TokenProfiler};
use pi_session::{Role, SessionTree};
use pi_tools::{ToolCall, ToolExecutor, ToolResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub mod alfred;
pub mod ckg;
pub mod coedit;
pub mod crystallize;
pub mod federation;
pub mod firstmate;
pub mod herdr;
pub mod plan;
pub mod skills;
pub mod speculate;
pub mod subagents;
pub mod sync;
pub mod undo;
pub mod vault;

pub use alfred::{AlfredAdvisory, AlfredProtocol, EscalationLevel, ValueStatement};
pub use crystallize::SkillCrystallizer;
pub use federation::{FederatedFleet, SpecialistIdentity, SpecialistInfo};
pub use firstmate::{
    CrewBackend, CrewMergeMode, CrewTask, CrewTaskShape, CrewTaskStatus, FirstMateDistro,
};
pub use herdr::{HerdrAgentState, HerdrEnvironment, HerdrProtocol};
pub use pi_tools::{Hook, HookRegistry, LifecycleEvent, set_global_hook_registry};
pub use pi_tools::{McpManager, McpServerConfig, McpToolDefinition, get_mcp_manager};
pub use plan::{ExecutionPlan, PlanExecutor, PlanTask, TaskStatus};
pub use skills::{SkillDefinition, SkillRegistry};
pub use speculate::{
    ArbitrationDecision, SpeculativeBranchResult, SpeculativeEngine, SpeculativeRaceResult,
    SpeculativeStatus, SpeculativeStrategy,
};
pub use subagents::{
    SubagentConfig, SubagentInstance, SubagentManager, SubagentRunner, SubagentStatus,
    SubagentSummary,
};
pub use sync::StateSynchronizer;
pub use undo::{ActionSnapshot, ActionSnapshotKind, UndoEngine};
pub use vault::{MemoryEntry, ReflexionEngine, TauVault};

pub const DEFAULT_PI_SYSTEM_PROMPT: &str = r#"You are Pi, a minimal, fast, and capable AI coding agent.
Your primary goal is to help the user write, debug, refactor, and maintain code cleanly.

Operating Principles:
1. Simplicity First: Write minimum code that solves the problem. No unnecessary abstractions.
2. Surgical Changes: Touch only what the task requires. Don't refactor adjacent code unless asked.
3. Verification: Verify changes by testing or running linters before claiming done.
4. Transparency: Be direct, concise, and technical. Avoid filler language.

You have access to native tools:
- read: View contents or line slices of files
- write: Create or overwrite files
- edit: Make precise find-and-replace edits to existing files
- bash: Execute shell commands to build, test, or inspect system state
- grep: Search directory files for a pattern or regular expression
- find: Find files by filename or glob pattern
- ls: List directory contents with file metadata and sizes
- web_fetch: Fetch URLs and extract clean markdown content
- web_search: Search the live web for current information, docs, or news
- git: Perform git status checks, diffs, log inspection, commit synthesis, and worktree workspace isolation
- github: Inspect GitHub pull requests, issues, and workflow runs
- lsp: Query language server for compiler diagnostics, document symbols, definitions, and hover docs
- ast: Syntactically slice symbols, functions, classes, or outline file structure without guessing lines
- invoke_subagent: Spawn an autonomous background subagent with isolated context to perform a dedicated task
- manage_subagents: Inspect, list, query status, or cancel active background subagents
- crew_dispatch: Dispatch a First Mate crew task (Ship in isolated worktree, or Scout investigation)
- crew_status: Query status and reconciliation of active fleet tasks
- crew_merge: Review and merge a completed Ship task worktree back into target branch
- speculate: Run a speculative execution race testing two competing implementation strategies concurrently in ghost worktrees
"#;

#[derive(Debug, Clone)]
pub struct SystemPromptEngine {
    pub base_prompt: String,
    pub agents_md: Option<String>,
    pub skill_registry: SkillRegistry,
    pub vault: TauVault,
}

impl Default for SystemPromptEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemPromptEngine {
    pub fn new() -> Self {
        Self::with_vault(TauVault::new())
    }

    pub fn with_vault(vault: TauVault) -> Self {
        let agents_md = fs::read_to_string("AGENTS.md")
            .or_else(|_| fs::read_to_string(".agents/AGENTS.md"))
            .or_else(|_| fs::read_to_string(".pi/AGENTS.md"))
            .or_else(|_| fs::read_to_string("CLAUDE.md"))
            .or_else(|_| fs::read_to_string(".cursorrules"))
            .ok();
        Self {
            base_prompt: DEFAULT_PI_SYSTEM_PROMPT.to_string(),
            agents_md,
            skill_registry: SkillRegistry::new(),
            vault,
        }
    }

    pub fn build_prompt_with_hindsight(&self, query: &str) -> String {
        let mut full = self.base_prompt.clone();
        if let Some(ref agents) = self.agents_md {
            full.push_str("\n\n--- Project Instructions (AGENTS.md) ---\n");
            full.push_str(agents);
        }
        full.push_str(&self.skill_registry.format_prompt_summary());
        let hindsight = self.vault.format_hindsight_prompt(query);
        if !hindsight.is_empty() {
            full.push_str("\n\n");
            full.push_str(&hindsight);
        }
        full
    }

    pub fn build_full_prompt(&self) -> String {
        self.build_prompt_with_hindsight("")
    }

    pub fn build_prompt_for_turn(&self, query: &str) -> String {
        self.build_prompt_with_hindsight(query)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TurnEvent {
    ContextPrepared {
        token_estimate: usize,
    },
    ModelStreaming {
        chunk: String,
    },
    ToolExecuting {
        tool_name: String,
        tool_call_id: String,
    },
    ToolCompleted {
        tool_name: String,
        is_error: bool,
    },
    ContextCompacted {
        old_turns: usize,
        new_summary_len: usize,
    },
    TurnCompleted {
        total_tokens: usize,
    },
}

#[derive(Clone)]
pub struct AgentLoop {
    pub system_engine: SystemPromptEngine,
    pub session_tree: SessionTree,
    pub model_config: ModelConfig,
    pub max_context_tokens: usize,
    pub max_tool_iterations: usize,
    pub allowed_tools: Option<Vec<String>>,
    pub execution_plan: Option<ExecutionPlan>,
}

impl AgentLoop {
    pub fn new(model_config: ModelConfig) -> Self {
        let max_context_tokens = if model_config.context_window > 0 {
            model_config.context_window
        } else {
            128_000
        };
        Self {
            system_engine: SystemPromptEngine::new(),
            session_tree: SessionTree::new(),
            model_config,
            max_context_tokens,
            max_tool_iterations: 5,
            allowed_tools: None,
            execution_plan: None,
        }
    }

    pub fn with_allowed_tools(mut self, tools: Vec<String>) -> Self {
        self.allowed_tools = Some(tools);
        self
    }

    pub fn with_execution_plan(mut self, plan: ExecutionPlan) -> Self {
        self.execution_plan = Some(plan);
        self
    }

    pub fn set_execution_plan(&mut self, plan: ExecutionPlan) {
        self.execution_plan = Some(plan);
    }

    pub fn get_execution_plan(&self) -> Option<&ExecutionPlan> {
        self.execution_plan.as_ref()
    }

    pub fn get_execution_plan_mut(&mut self) -> Option<&mut ExecutionPlan> {
        self.execution_plan.as_mut()
    }

    pub fn format_plan_markdown(&self) -> Option<String> {
        self.execution_plan
            .as_ref()
            .map(ExecutionPlan::to_markdown_checklist)
    }

    pub fn crystallize_active_session(
        &mut self,
        skill_name: &str,
        description: &str,
    ) -> Result<std::path::PathBuf> {
        let history = self.session_tree.get_active_branch_history();
        let base_dir = dirs::home_dir()
            .map(|h| h.join(".tau").join("skills"))
            .unwrap_or_else(|| PathBuf::from(".tau/skills"));
        let (path, _) = SkillCrystallizer::crystallize_and_register_with_vault(
            &mut self.system_engine.skill_registry,
            &self.system_engine.vault,
            &history,
            skill_name,
            description,
            &base_dir,
        )?;
        Ok(path)
    }

    fn build_conversation_messages(&self) -> Vec<ChatMessage> {
        let history = self.session_tree.get_active_branch_history();
        let mut messages = Vec::new();

        for node in history {
            match node.role {
                Role::System => {
                    if node.content.starts_with("[Context Compaction Summary") {
                        messages.push(ChatMessage::system(&node.content));
                    }
                }
                Role::User => {
                    messages.push(ChatMessage::user(&node.content));
                }
                Role::Assistant => {
                    if let Some(ref tc) = node.tool_calls {
                        messages.push(ChatMessage::assistant_with_tool_calls(
                            &node.content,
                            tc.clone(),
                        ));
                    } else {
                        messages.push(ChatMessage::assistant(&node.content));
                    }
                }
                Role::Tool => {
                    let call_id = node
                        .tool_call_id
                        .clone()
                        .unwrap_or_else(|| "call_fallback".to_string());
                    let tool_name = node.tool_name.clone().unwrap_or_default();
                    messages.push(ChatMessage::tool_result(call_id, tool_name, &node.content));
                }
            }
        }

        messages
    }

    fn build_conversation_prompt(&self) -> String {
        let history = self.session_tree.get_active_branch_history();
        let mut prompt = String::new();

        for node in history {
            match node.role {
                Role::System => {
                    if node.content.contains("Context Compaction Summary") {
                        prompt.push_str(&format!("System Summary: {}\n\n", node.content));
                    }
                }
                Role::User => {
                    prompt.push_str(&format!("User: {}\n\n", node.content));
                }
                Role::Assistant => {
                    prompt.push_str(&format!("Assistant: {}\n\n", node.content));
                }
                Role::Tool => {
                    prompt.push_str(&format!("Tool Output: {}\n\n", node.content));
                }
            }
        }

        prompt
    }

    pub async fn compact_history_if_needed<F>(&mut self, event_tx: &mut F) -> Result<()>
    where
        F: FnMut(TurnEvent) + Send,
    {
        let system_prompt = self.system_engine.build_full_prompt();
        let history_text = self.build_conversation_prompt();
        let budget = TokenProfiler::compute_budget(
            &system_prompt,
            &history_text,
            &self.model_config.model_id,
            self.max_context_tokens,
        );

        if !budget.needs_compaction {
            return Ok(());
        }

        let history = self.session_tree.get_active_branch_history();
        if history.len() < 6 {
            return Ok(());
        }

        // Find turn cut boundary (aligned to a User message so to_keep starts cleanly)
        let target_cut = history.len() / 2;
        let mut cut_idx = None;
        for (i, node) in history.iter().enumerate().take(target_cut + 3) {
            if i >= 1 && node.role == Role::User {
                cut_idx = Some(i);
            }
        }
        if cut_idx.is_none() {
            for (i, node) in history.iter().enumerate().skip(target_cut) {
                if node.role == Role::User {
                    cut_idx = Some(i);
                    break;
                }
            }
        }
        let cut_idx = cut_idx.unwrap_or(target_cut);

        let to_summarize = &history[..cut_idx];
        let to_keep = &history[cut_idx..];

        let mut transcript_to_summarize = String::new();
        for node in to_summarize {
            match node.role {
                Role::User => {
                    transcript_to_summarize.push_str(&format!("User: {}\n\n", node.content))
                }
                Role::Assistant => {
                    transcript_to_summarize.push_str(&format!("Assistant: {}\n\n", node.content))
                }
                Role::Tool => {
                    transcript_to_summarize.push_str(&format!("Tool: {}\n\n", node.content))
                }
                Role::System => {
                    if node.content.contains("Context Compaction Summary") {
                        transcript_to_summarize
                            .push_str(&format!("Previous History Summary:\n{}\n\n", node.content));
                    }
                }
            }
        }

        let summary_prompt = format!(
            "Please provide a concise, factual summary of the following conversation history for context preservation. \
            Include key tasks, decisions, files modified, and current status:\n\n{}",
            transcript_to_summarize
        );

        let summary_system =
            "You are a context compaction engine. Produce a dense, factual summary.";
        let summary_result = match ProviderClient::stream_messages_with_tools(
            &self.model_config,
            summary_system,
            &[ChatMessage::user(&summary_prompt)],
            &[],
            |_| {},
        )
        .await
        {
            Ok(resp) if !resp.text.trim().is_empty() => resp.text,
            _ => format!(
                "Previous conversation covered {} turns involving task discussion and tool operations.",
                to_summarize.len()
            ),
        };

        let summary_content = format!(
            "[Context Compaction Summary - Previous {} Turns]\n{}\n[End Context Summary]",
            to_summarize.len(),
            summary_result.trim()
        );
        let summary_len = summary_content.len();
        let old_turns = to_summarize.len();

        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string());
        let mut new_tree = SessionTree::new_with_disk_path(
            self.session_tree.disk_path.clone(),
            &cwd,
            self.session_tree.session_id.clone(),
        );

        new_tree.append_child(Role::System, summary_content);

        for node in to_keep {
            new_tree.append_child_with_metadata(
                node.role,
                node.content.clone(),
                node.tool_call_id.clone(),
                node.tool_name.clone(),
                node.tool_calls.clone(),
            );
        }

        self.session_tree = new_tree;

        event_tx(TurnEvent::ContextCompacted {
            old_turns,
            new_summary_len: summary_len,
        });

        Ok(())
    }

    pub async fn run_turn<F>(&mut self, user_input: &str, mut event_tx: F) -> Result<String>
    where
        F: FnMut(TurnEvent) + Send,
    {
        HerdrProtocol::emit_state(HerdrAgentState::Working);
        self.session_tree
            .append_child(Role::User, user_input.to_string());

        let _ = self
            .emit_hook_event(LifecycleEvent::TurnStarted {
                prompt: user_input.to_string(),
            })
            .await;

        let _ = self.compact_history_if_needed(&mut event_tx).await;

        let mut full_system_prompt = self.system_engine.build_prompt_for_turn(user_input);
        if let Some(ref plan) = self.execution_plan {
            full_system_prompt.push_str("\n\n--- Active Execution Plan ---\n");
            full_system_prompt.push_str(&plan.to_markdown_checklist());
        }
        let mut turn_iteration = 0;
        let mut final_response = String::new();

        while turn_iteration < self.max_tool_iterations {
            turn_iteration += 1;

            let conversation_messages = self.build_conversation_messages();
            let conversation_prompt = self.build_conversation_prompt();
            let token_estimate =
                TokenProfiler::estimate_tokens(&conversation_prompt, &self.model_config.model_id);
            event_tx(TurnEvent::ContextPrepared { token_estimate });

            let tool_defs = {
                let all_defs = ToolExecutor::tool_definitions();
                if let Some(ref allowed) = self.allowed_tools {
                    all_defs
                        .into_iter()
                        .filter(|td| {
                            let name = td
                                .get("function")
                                .and_then(|f| f.get("name"))
                                .and_then(|n| n.as_str())
                                .or_else(|| td.get("name").and_then(|n| n.as_str()))
                                .unwrap_or("");
                            allowed.iter().any(|a| a.eq_ignore_ascii_case(name))
                        })
                        .collect()
                } else {
                    all_defs
                }
            };

            let provider_resp = match ProviderClient::stream_messages_with_tools(
                &self.model_config,
                &full_system_prompt,
                &conversation_messages,
                &tool_defs,
                |chunk| {
                    event_tx(TurnEvent::ModelStreaming { chunk });
                },
            )
            .await
            {
                Ok(resp) => resp,
                Err(e) => {
                    ReflexionEngine::distill_turn_error(
                        &self.system_engine.vault,
                        user_input,
                        &e.to_string(),
                    );
                    return Err(e);
                }
            };

            let response_text = provider_resp.text;

            // Dual Tool Protocol: Prefer structured JSON tool calls, fallback to markdown parsing
            let tool_calls_to_execute: Vec<ToolCall> = if !provider_resp.tool_calls.is_empty() {
                let calls: Vec<ToolCall> = provider_resp
                    .tool_calls
                    .into_iter()
                    .map(|call| ToolCall {
                        id: call.id,
                        name: call.name,
                        arguments: call.arguments,
                    })
                    .collect();
                if let Some(ref allowed) = self.allowed_tools {
                    calls
                        .into_iter()
                        .filter(|c| allowed.iter().any(|a| a.eq_ignore_ascii_case(&c.name)))
                        .collect()
                } else {
                    calls
                }
            } else {
                let calls = Self::extract_fallback_tool_calls(&response_text);
                if let Some(ref allowed) = self.allowed_tools {
                    calls
                        .into_iter()
                        .filter(|c| allowed.iter().any(|a| a.eq_ignore_ascii_case(&c.name)))
                        .collect()
                } else {
                    calls
                }
            };

            if !tool_calls_to_execute.is_empty() {
                let raw_tool_calls_json: serde_json::Value = serde_json::Value::Array(
                    tool_calls_to_execute
                        .iter()
                        .map(|c| {
                            serde_json::json!({
                                "id": c.id,
                                "type": "function",
                                "function": {
                                    "name": c.name,
                                    "arguments": serde_json::to_string(&c.arguments).unwrap_or_default()
                                }
                            })
                        })
                        .collect(),
                );

                self.session_tree.append_child_with_metadata(
                    Role::Assistant,
                    response_text.clone(),
                    None,
                    None,
                    Some(raw_tool_calls_json),
                );

                let mut tool_outputs = Vec::new();

                for call in tool_calls_to_execute {
                    event_tx(TurnEvent::ToolExecuting {
                        tool_name: call.name.clone(),
                        tool_call_id: call.id.clone(),
                    });
                    HerdrProtocol::emit_state(HerdrAgentState::Working);

                    let tool_res: ToolResult = ToolExecutor::execute(&call).await;

                    event_tx(TurnEvent::ToolCompleted {
                        tool_name: call.name.clone(),
                        is_error: tool_res.is_error,
                    });

                    if tool_res.is_error {
                        ReflexionEngine::distill_tool_failure(
                            &self.system_engine.vault,
                            &call.name,
                            &call.arguments,
                            &tool_res.output,
                        );
                    }

                    self.session_tree.append_child_with_metadata(
                        Role::Tool,
                        tool_res.output.clone(),
                        Some(tool_res.tool_call_id.clone()),
                        Some(call.name),
                        None,
                    );
                    tool_outputs.push(tool_res.output);
                }

                if response_text.is_empty() {
                    final_response = format!("Tool Output:\n{}", tool_outputs.join("\n---\n"));
                } else {
                    final_response = format!(
                        "{}\n\nTool Output:\n{}",
                        response_text,
                        tool_outputs.join("\n---\n")
                    );
                }
            } else {
                self.session_tree
                    .append_child(Role::Assistant, response_text.clone());
                final_response = response_text;
                break;
            }
        }

        let total_tokens = TokenProfiler::estimate_tokens(
            &self.build_conversation_prompt(),
            &self.model_config.model_id,
        );
        event_tx(TurnEvent::TurnCompleted { total_tokens });
        let _ = self
            .emit_hook_event(LifecycleEvent::TurnFinished {
                ok: final_response.is_empty(),
            })
            .await;
        HerdrProtocol::emit_state(HerdrAgentState::Done);
        HerdrProtocol::emit_state(HerdrAgentState::Idle);
        Ok(final_response)
    }

    async fn emit_hook_event(&self, event: LifecycleEvent) {
        let registry = pi_tools::global_hook_registry();
        if let Some(registry) = registry {
            registry.emit(&event).await;
        }
    }

    pub fn tokenize_args(input: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let mut current = String::new();
        let mut in_single = false;
        let mut in_double = false;
        let mut chars = input.chars().peekable();

        while let Some(c) = chars.next() {
            match c {
                '\'' if !in_double => {
                    in_single = !in_single;
                }
                '"' if !in_single => {
                    in_double = !in_double;
                }
                '\\' => {
                    if let Some(next_c) = chars.next() {
                        current.push(next_c);
                    }
                }
                c if c.is_whitespace() && !in_single && !in_double => {
                    if !current.is_empty() {
                        tokens.push(current.clone());
                        current.clear();
                    }
                }
                c => {
                    current.push(c);
                }
            }
        }

        if !current.is_empty() {
            tokens.push(current);
        }

        tokens
    }

    /// Backwards-compatible bash block extractor
    pub fn extract_bash_command(text: &str) -> Option<&str> {
        if let Some(start) = text.find("```bash") {
            let rest = &text[start + 7..];
            if let Some(end) = rest.find("```") {
                let cmd = rest[..end].trim();
                if !cmd.is_empty() {
                    return Some(cmd);
                }
            }
        }
        None
    }

    /// Extract fallback markdown tool invocations from LLM response text
    pub fn extract_fallback_tool_calls(text: &str) -> Vec<ToolCall> {
        let mut calls = Vec::new();
        let mut pos = 0;

        while let Some(start_idx) = text[pos..].find("```") {
            let block_start = pos + start_idx + 3;
            let rest = &text[block_start..];
            let Some(newline_offset) = rest.find('\n') else {
                break;
            };
            let tag_line = rest[..newline_offset].trim();
            let body_start = block_start + newline_offset + 1;

            let Some(closing_offset) = text[body_start..].find("```") else {
                break;
            };
            let body = &text[body_start..body_start + closing_offset];
            pos = body_start + closing_offset + 3;

            if let Some(tool_call) = Self::parse_markdown_block(tag_line, body) {
                calls.push(tool_call);
            }
        }

        calls
    }

    fn parse_markdown_block(tag_line: &str, body: &str) -> Option<ToolCall> {
        let trimmed_tag = tag_line.trim();
        let call_id = uuid::Uuid::new_v4().to_string();

        if trimmed_tag == "bash"
            || trimmed_tag == "sh"
            || trimmed_tag == "shell"
            || trimmed_tag.starts_with("bash ")
            || trimmed_tag.starts_with("sh ")
            || trimmed_tag.starts_with("shell ")
        {
            let cmd = body.trim();
            if !cmd.is_empty() {
                return Some(ToolCall {
                    id: call_id,
                    name: "bash".to_string(),
                    arguments: serde_json::json!({ "command": cmd }),
                });
            }
        } else if trimmed_tag == "write" || trimmed_tag.starts_with("write ") {
            let rest = trimmed_tag
                .strip_prefix("write")
                .map(str::trim)
                .unwrap_or("");
            let tokens = Self::tokenize_args(rest);
            let path = tokens.first().cloned().unwrap_or_default();
            if !path.is_empty() {
                let content = body
                    .strip_suffix("\r\n")
                    .or_else(|| body.strip_suffix('\n'))
                    .unwrap_or(body);
                return Some(ToolCall {
                    id: call_id,
                    name: "write".to_string(),
                    arguments: serde_json::json!({
                        "path": path,
                        "content": content
                    }),
                });
            } else if let Ok(val) = serde_json::from_str::<serde_json::Value>(body.trim()) {
                if val.get("path").is_some() && val.get("content").is_some() {
                    return Some(ToolCall {
                        id: call_id,
                        name: "write".to_string(),
                        arguments: val,
                    });
                }
            } else if let Some((first_line, rest)) = body.split_once('\n') {
                let p = first_line.trim().trim_matches('"').trim_matches('\'');
                if !p.is_empty() {
                    let content = rest
                        .strip_suffix("\r\n")
                        .or_else(|| rest.strip_suffix('\n'))
                        .unwrap_or(rest);
                    return Some(ToolCall {
                        id: call_id,
                        name: "write".to_string(),
                        arguments: serde_json::json!({
                            "path": p,
                            "content": content
                        }),
                    });
                }
            }
        } else if trimmed_tag == "edit" || trimmed_tag.starts_with("edit ") {
            let rest = trimmed_tag
                .strip_prefix("edit")
                .map(str::trim)
                .unwrap_or("");
            let tokens = Self::tokenize_args(rest);
            let path = tokens.first().cloned().unwrap_or_default();

            // 1. Check if body is JSON
            if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(body.trim())
                && val.is_object()
            {
                if !path.is_empty() && val.get("path").is_none() {
                    val["path"] = serde_json::Value::String(path.clone());
                }
                let has_target = val.get("target").is_some() || val.get("oldText").is_some();
                let has_replacement =
                    val.get("replacement").is_some() || val.get("newText").is_some();
                if val.get("path").is_some() && has_target && has_replacement {
                    return Some(ToolCall {
                        id: call_id,
                        name: "edit".to_string(),
                        arguments: val,
                    });
                }
            }

            // 2. Check if body has ==== delimiter
            let mut target_lines = Vec::new();
            let mut replacement_lines = Vec::new();
            let mut found_delim = false;

            for line in body.lines() {
                let trimmed = line.trim();
                if !found_delim
                    && (trimmed == "===="
                        || (trimmed.starts_with("===") && trimmed.chars().all(|c| c == '=')))
                {
                    found_delim = true;
                } else if !found_delim {
                    target_lines.push(line);
                } else {
                    replacement_lines.push(line);
                }
            }

            if found_delim && !path.is_empty() {
                let target = target_lines.join("\n");
                let replacement = replacement_lines.join("\n");
                return Some(ToolCall {
                    id: call_id,
                    name: "edit".to_string(),
                    arguments: serde_json::json!({
                        "path": path,
                        "target": target,
                        "replacement": replacement
                    }),
                });
            }
        } else if trimmed_tag == "read" || trimmed_tag.starts_with("read ") {
            let rest = trimmed_tag
                .strip_prefix("read")
                .map(str::trim)
                .unwrap_or("");

            if let Ok(val) = serde_json::from_str::<serde_json::Value>(body.trim())
                && val.get("path").is_some()
            {
                return Some(ToolCall {
                    id: call_id,
                    name: "read".to_string(),
                    arguments: val,
                });
            }

            if !rest.is_empty() {
                let tokens = Self::tokenize_args(rest);
                if !tokens.is_empty() {
                    let path = &tokens[0];
                    let mut args = serde_json::json!({ "path": path });
                    if tokens.len() >= 2
                        && let Ok(start) = tokens[1].parse::<u64>()
                    {
                        args["start_line"] = serde_json::json!(start);
                    }
                    if tokens.len() >= 3
                        && let Ok(end) = tokens[2].parse::<u64>()
                    {
                        args["end_line"] = serde_json::json!(end);
                    }
                    return Some(ToolCall {
                        id: call_id,
                        name: "read".to_string(),
                        arguments: args,
                    });
                }
            } else {
                let trimmed_body = body.trim();
                if !trimmed_body.is_empty() {
                    let path = trimmed_body
                        .lines()
                        .next()
                        .unwrap_or("")
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'');
                    if !path.is_empty() {
                        return Some(ToolCall {
                            id: call_id,
                            name: "read".to_string(),
                            arguments: serde_json::json!({ "path": path }),
                        });
                    }
                }
            }
        } else if trimmed_tag == "grep" || trimmed_tag.starts_with("grep ") {
            let rest = trimmed_tag
                .strip_prefix("grep")
                .map(str::trim)
                .unwrap_or("");
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(body.trim())
                && val.get("pattern").is_some()
            {
                return Some(ToolCall {
                    id: call_id,
                    name: "grep".to_string(),
                    arguments: val,
                });
            }
            if !rest.is_empty() {
                let tokens = Self::tokenize_args(rest);
                if !tokens.is_empty() {
                    let pattern = &tokens[0];
                    let path = tokens.get(1).map(|s| s.as_str()).unwrap_or(".");
                    return Some(ToolCall {
                        id: call_id,
                        name: "grep".to_string(),
                        arguments: serde_json::json!({ "pattern": pattern, "path": path }),
                    });
                }
            } else {
                let trimmed_body = body.trim();
                if !trimmed_body.is_empty() {
                    let pattern = trimmed_body
                        .lines()
                        .next()
                        .unwrap_or("")
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'');
                    if !pattern.is_empty() {
                        return Some(ToolCall {
                            id: call_id,
                            name: "grep".to_string(),
                            arguments: serde_json::json!({ "pattern": pattern, "path": "." }),
                        });
                    }
                }
            }
        } else if trimmed_tag == "find" || trimmed_tag.starts_with("find ") {
            let rest = trimmed_tag
                .strip_prefix("find")
                .map(str::trim)
                .unwrap_or("");
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(body.trim())
                && val.get("pattern").is_some()
            {
                return Some(ToolCall {
                    id: call_id,
                    name: "find".to_string(),
                    arguments: val,
                });
            }
            if !rest.is_empty() {
                let tokens = Self::tokenize_args(rest);
                if !tokens.is_empty() {
                    let pattern = &tokens[0];
                    let path = tokens.get(1).map(|s| s.as_str()).unwrap_or(".");
                    return Some(ToolCall {
                        id: call_id,
                        name: "find".to_string(),
                        arguments: serde_json::json!({ "pattern": pattern, "path": path }),
                    });
                }
            } else {
                let trimmed_body = body.trim();
                if !trimmed_body.is_empty() {
                    let pattern = trimmed_body
                        .lines()
                        .next()
                        .unwrap_or("")
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'');
                    if !pattern.is_empty() {
                        return Some(ToolCall {
                            id: call_id,
                            name: "find".to_string(),
                            arguments: serde_json::json!({ "pattern": pattern, "path": "." }),
                        });
                    }
                }
            }
        } else if trimmed_tag == "ls" || trimmed_tag.starts_with("ls ") {
            let rest = trimmed_tag
                .strip_prefix("ls")
                .map(str::trim)
                .unwrap_or("")
                .trim_matches('"')
                .trim_matches('\'');
            let path = if rest.is_empty() { "." } else { rest };
            return Some(ToolCall {
                id: call_id,
                name: "ls".to_string(),
                arguments: serde_json::json!({ "path": path }),
            });
        } else if trimmed_tag == "web_fetch"
            || trimmed_tag == "web"
            || trimmed_tag.starts_with("web_fetch ")
            || trimmed_tag.starts_with("web ")
        {
            let rest = if let Some(r) = trimmed_tag.strip_prefix("web_fetch") {
                r.trim()
            } else {
                trimmed_tag.strip_prefix("web").map(str::trim).unwrap_or("")
            };

            if let Ok(val) = serde_json::from_str::<serde_json::Value>(body.trim())
                && val.get("url").is_some()
            {
                return Some(ToolCall {
                    id: call_id,
                    name: "web_fetch".to_string(),
                    arguments: val,
                });
            }

            let url = if !rest.is_empty() {
                rest.trim_matches('"').trim_matches('\'')
            } else {
                body.lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
            };

            if !url.is_empty() {
                return Some(ToolCall {
                    id: call_id,
                    name: "web_fetch".to_string(),
                    arguments: serde_json::json!({ "url": url }),
                });
            }
        } else if trimmed_tag == "web_search"
            || trimmed_tag == "search"
            || trimmed_tag.starts_with("web_search ")
            || trimmed_tag.starts_with("search ")
        {
            let rest = if let Some(r) = trimmed_tag.strip_prefix("web_search") {
                r.trim()
            } else {
                trimmed_tag
                    .strip_prefix("search")
                    .map(str::trim)
                    .unwrap_or("")
            };

            if let Ok(val) = serde_json::from_str::<serde_json::Value>(body.trim())
                && val.get("query").is_some()
            {
                return Some(ToolCall {
                    id: call_id,
                    name: "web_search".to_string(),
                    arguments: val,
                });
            }

            let query = if !rest.is_empty() {
                rest.trim_matches('"').trim_matches('\'')
            } else {
                body.lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
            };

            if !query.is_empty() {
                return Some(ToolCall {
                    id: call_id,
                    name: "web_search".to_string(),
                    arguments: serde_json::json!({ "query": query }),
                });
            }
        } else if trimmed_tag == "git"
            || trimmed_tag.starts_with("git ")
            || trimmed_tag.starts_with("git_")
        {
            let rest = if let Some(r) = trimmed_tag.strip_prefix("git ") {
                r.trim()
            } else if let Some(r) = trimmed_tag.strip_prefix("git_") {
                r.trim()
            } else {
                trimmed_tag.strip_prefix("git").map(str::trim).unwrap_or("")
            };

            if let Ok(val) = serde_json::from_str::<serde_json::Value>(body.trim())
                && val.get("action").is_some()
            {
                return Some(ToolCall {
                    id: call_id,
                    name: "git".to_string(),
                    arguments: val,
                });
            }

            let tokens = Self::tokenize_args(rest);
            let mut args = serde_json::json!({});

            if !tokens.is_empty() {
                let action = &tokens[0];
                args["action"] = serde_json::json!(action);

                match action.as_str() {
                    "worktree_add" | "worktree_create" => {
                        if let Some(task_id) = tokens.get(1) {
                            args["task_id"] = serde_json::json!(task_id);
                        }
                        if let Some(base) = tokens.get(2) {
                            args["base_branch"] = serde_json::json!(base);
                        }
                    }
                    "worktree_remove" | "worktree_delete" => {
                        if let Some(task_id) = tokens.get(1) {
                            args["task_id"] = serde_json::json!(task_id);
                        }
                        if tokens.iter().any(|t| t == "--force" || t == "-f") {
                            args["force"] = serde_json::json!(true);
                        }
                    }
                    "worktree_merge" => {
                        if let Some(task_id) = tokens.get(1) {
                            args["task_id"] = serde_json::json!(task_id);
                        }
                        if let Some(target) = tokens.get(2) {
                            args["target_branch"] = serde_json::json!(target);
                        }
                    }
                    "worktree_list" => {}
                    "commit" => {
                        if tokens.len() > 1 {
                            args["message"] = serde_json::json!(tokens[1..].join(" "));
                        } else {
                            let b = body.trim();
                            if !b.is_empty() {
                                args["message"] = serde_json::json!(b);
                            }
                        }
                    }
                    "diff" => {
                        if tokens.iter().any(|t| t == "--staged" || t == "--cached") {
                            args["staged"] = serde_json::json!(true);
                        }
                        if let Some(f) = tokens.iter().skip(1).find(|t| !t.starts_with('-')) {
                            args["file"] = serde_json::json!(f);
                        }
                    }
                    _ => {}
                }
            } else {
                let first_line = body.lines().next().unwrap_or("status").trim();
                let action = if first_line.is_empty() {
                    "status"
                } else {
                    first_line
                };
                args["action"] = serde_json::json!(action);
            }

            return Some(ToolCall {
                id: call_id,
                name: "git".to_string(),
                arguments: args,
            });
        } else if trimmed_tag == "github" || trimmed_tag.starts_with("github ") {
            let rest = trimmed_tag
                .strip_prefix("github")
                .map(str::trim)
                .unwrap_or("");

            if let Ok(val) = serde_json::from_str::<serde_json::Value>(body.trim())
                && val.get("action").is_some()
            {
                return Some(ToolCall {
                    id: call_id,
                    name: "github".to_string(),
                    arguments: val,
                });
            }

            let action = if !rest.is_empty() {
                rest.split_whitespace().next().unwrap_or("pr_list")
            } else {
                body.lines().next().unwrap_or("pr_list").trim()
            };

            return Some(ToolCall {
                id: call_id,
                name: "github".to_string(),
                arguments: serde_json::json!({ "action": action }),
            });
        } else if trimmed_tag == "lsp" || trimmed_tag.starts_with("lsp ") {
            let rest = trimmed_tag.strip_prefix("lsp").map(str::trim).unwrap_or("");

            if let Ok(val) = serde_json::from_str::<serde_json::Value>(body.trim())
                && val.get("action").is_some()
            {
                return Some(ToolCall {
                    id: call_id,
                    name: "lsp".to_string(),
                    arguments: val,
                });
            }

            let tokens = Self::tokenize_args(rest);
            let action = tokens.first().map(|s| s.as_str()).unwrap_or("diagnostics");
            let path = tokens.get(1).map(|s| s.as_str()).unwrap_or(".");
            let symbol = tokens.get(2).map(|s| s.as_str());

            let mut args = serde_json::json!({ "action": action, "path": path });
            if let Some(s) = symbol {
                args["symbol"] = serde_json::json!(s);
            }

            return Some(ToolCall {
                id: call_id,
                name: "lsp".to_string(),
                arguments: args,
            });
        } else if trimmed_tag == "ast"
            || trimmed_tag.starts_with("ast ")
            || trimmed_tag == "ast_slice"
            || trimmed_tag.starts_with("ast_slice ")
        {
            let rest = if let Some(r) = trimmed_tag.strip_prefix("ast_slice") {
                r.trim()
            } else {
                trimmed_tag.strip_prefix("ast").map(str::trim).unwrap_or("")
            };

            if let Ok(val) = serde_json::from_str::<serde_json::Value>(body.trim())
                && val.get("path").is_some()
            {
                return Some(ToolCall {
                    id: call_id,
                    name: "ast".to_string(),
                    arguments: val,
                });
            }

            let tokens = Self::tokenize_args(rest);
            let path = tokens.first().map(|s| s.as_str()).unwrap_or("");
            let symbol = tokens.get(1).map(|s| s.as_str());

            let mut args = serde_json::json!({ "path": path });
            if let Some(s) = symbol {
                args["symbol"] = serde_json::json!(s);
            }

            return Some(ToolCall {
                id: call_id,
                name: "ast".to_string(),
                arguments: args,
            });
        } else if trimmed_tag == "invoke_subagent"
            || trimmed_tag == "subagent"
            || trimmed_tag.starts_with("invoke_subagent ")
            || trimmed_tag.starts_with("subagent ")
        {
            let rest = if let Some(r) = trimmed_tag.strip_prefix("invoke_subagent") {
                r.trim()
            } else {
                trimmed_tag
                    .strip_prefix("subagent")
                    .map(str::trim)
                    .unwrap_or("")
            };

            if let Ok(val) = serde_json::from_str::<serde_json::Value>(body.trim())
                && val.get("name").is_some()
                && val.get("task").is_some()
            {
                return Some(ToolCall {
                    id: call_id,
                    name: "invoke_subagent".to_string(),
                    arguments: val,
                });
            }

            let tokens = Self::tokenize_args(rest);
            let name = tokens
                .first()
                .cloned()
                .unwrap_or_else(|| "Worker".to_string());
            let task = if !body.trim().is_empty() {
                body.trim().to_string()
            } else if tokens.len() > 1 {
                tokens[1..].join(" ")
            } else {
                "Perform task".to_string()
            };

            return Some(ToolCall {
                id: call_id,
                name: "invoke_subagent".to_string(),
                arguments: serde_json::json!({
                    "name": name,
                    "task": task
                }),
            });
        } else if trimmed_tag == "manage_subagents" || trimmed_tag.starts_with("manage_subagents ")
        {
            let rest = trimmed_tag
                .strip_prefix("manage_subagents")
                .map(str::trim)
                .unwrap_or("");

            if let Ok(val) = serde_json::from_str::<serde_json::Value>(body.trim())
                && val.get("action").is_some()
            {
                return Some(ToolCall {
                    id: call_id,
                    name: "manage_subagents".to_string(),
                    arguments: val,
                });
            }

            let tokens = Self::tokenize_args(rest);
            let action = tokens.first().map(|s| s.as_str()).unwrap_or("list");
            let id = tokens.get(1).map(|s| s.as_str());

            let mut args = serde_json::json!({ "action": action });
            if let Some(subagent_id) = id {
                args["id"] = serde_json::json!(subagent_id);
            }

            return Some(ToolCall {
                id: call_id,
                name: "manage_subagents".to_string(),
                arguments: args,
            });
        } else if trimmed_tag == "crew_dispatch" || trimmed_tag.starts_with("crew_dispatch ") {
            let rest = trimmed_tag
                .strip_prefix("crew_dispatch")
                .map(str::trim)
                .unwrap_or("");

            if let Ok(val) = serde_json::from_str::<serde_json::Value>(body.trim())
                && val.get("task").is_some()
            {
                return Some(ToolCall {
                    id: call_id,
                    name: "crew_dispatch".to_string(),
                    arguments: val,
                });
            }

            let tokens = Self::tokenize_args(rest);
            let shape = tokens.first().map(|s| s.as_str()).unwrap_or("ship");
            let task = if !body.trim().is_empty() {
                body.trim().to_string()
            } else if tokens.len() > 1 {
                tokens[1..].join(" ")
            } else {
                "Perform task".to_string()
            };

            return Some(ToolCall {
                id: call_id,
                name: "crew_dispatch".to_string(),
                arguments: serde_json::json!({
                    "shape": shape,
                    "task": task
                }),
            });
        } else if trimmed_tag == "crew_status" || trimmed_tag.starts_with("crew_status ") {
            let rest = trimmed_tag
                .strip_prefix("crew_status")
                .map(str::trim)
                .unwrap_or("");

            if let Ok(val) = serde_json::from_str::<serde_json::Value>(body.trim()) {
                return Some(ToolCall {
                    id: call_id,
                    name: "crew_status".to_string(),
                    arguments: val,
                });
            }

            let tokens = Self::tokenize_args(rest);
            let action = tokens.first().map(|s| s.as_str()).unwrap_or("list");
            let task_id = tokens.get(1).map(|s| s.as_str());

            let mut args = serde_json::json!({ "action": action });
            if let Some(tid) = task_id {
                args["task_id"] = serde_json::json!(tid);
            }

            return Some(ToolCall {
                id: call_id,
                name: "crew_status".to_string(),
                arguments: args,
            });
        } else if trimmed_tag == "crew_merge" || trimmed_tag.starts_with("crew_merge ") {
            let rest = trimmed_tag
                .strip_prefix("crew_merge")
                .map(str::trim)
                .unwrap_or("");

            if let Ok(val) = serde_json::from_str::<serde_json::Value>(body.trim())
                && val.get("task_id").is_some()
            {
                return Some(ToolCall {
                    id: call_id,
                    name: "crew_merge".to_string(),
                    arguments: val,
                });
            }

            let tokens = Self::tokenize_args(rest);
            let task_id = tokens.first().map(|s| s.as_str()).unwrap_or("");
            let target_branch = tokens.get(1).map(|s| s.as_str()).unwrap_or("HEAD");

            return Some(ToolCall {
                id: call_id,
                name: "crew_merge".to_string(),
                arguments: serde_json::json!({
                    "task_id": task_id,
                    "target_branch": target_branch
                }),
            });
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn test_system_prompt_builder() {
        let engine = SystemPromptEngine {
            base_prompt: "Base system prompt".to_string(),
            agents_md: Some("Custom AGENTS guidelines".to_string()),
            skill_registry: SkillRegistry::default(),
            vault: TauVault::open_in_memory().unwrap(),
        };
        let full = engine.build_full_prompt();
        assert!(full.contains("Base system prompt"));
        assert!(full.contains("Custom AGENTS guidelines"));
    }

    #[test]
    fn test_extract_fallback_web_git_github() {
        let text = r#"
```web_fetch https://example.com
```

```git status
```

```github pr_list
```

```lsp symbols src/lib.rs
```

```ast src/main.rs run
```
"#;
        let calls = AgentLoop::extract_fallback_tool_calls(text);
        assert_eq!(calls.len(), 5);
        assert_eq!(calls[0].name, "web_fetch");
        assert_eq!(calls[0].arguments["url"], "https://example.com");
        assert_eq!(calls[1].name, "git");
        assert_eq!(calls[1].arguments["action"], "status");
        assert_eq!(calls[2].name, "github");
        assert_eq!(calls[2].arguments["action"], "pr_list");
        assert_eq!(calls[3].name, "lsp");
        assert_eq!(calls[3].arguments["action"], "symbols");
        assert_eq!(calls[3].arguments["path"], "src/lib.rs");
        assert_eq!(calls[4].name, "ast");
        assert_eq!(calls[4].arguments["path"], "src/main.rs");
        assert_eq!(calls[4].arguments["symbol"], "run");
    }

    #[test]
    fn test_extract_fallback_git_worktree() {
        let text = r#"
```git worktree_add task-100 feature-branch
```

```git worktree_remove task-100 --force
```

```git worktree_merge task-100 main
```

```git worktree_list
```
"#;
        let calls = AgentLoop::extract_fallback_tool_calls(text);
        assert_eq!(calls.len(), 4);
        assert_eq!(calls[0].name, "git");
        assert_eq!(calls[0].arguments["action"], "worktree_add");
        assert_eq!(calls[0].arguments["task_id"], "task-100");
        assert_eq!(calls[0].arguments["base_branch"], "feature-branch");

        assert_eq!(calls[1].name, "git");
        assert_eq!(calls[1].arguments["action"], "worktree_remove");
        assert_eq!(calls[1].arguments["task_id"], "task-100");
        assert_eq!(calls[1].arguments["force"], true);

        assert_eq!(calls[2].name, "git");
        assert_eq!(calls[2].arguments["action"], "worktree_merge");
        assert_eq!(calls[2].arguments["task_id"], "task-100");
        assert_eq!(calls[2].arguments["target_branch"], "main");

        assert_eq!(calls[3].name, "git");
        assert_eq!(calls[3].arguments["action"], "worktree_list");
    }

    #[test]
    fn test_extract_bash_command() {
        let text = "Here is the command:\n```bash\ncargo test\n```\nDone!";
        let cmd = AgentLoop::extract_bash_command(text);
        assert_eq!(cmd, Some("cargo test"));

        let no_tool = "No code blocks here.";
        assert_eq!(AgentLoop::extract_bash_command(no_tool), None);
    }

    #[test]
    fn test_extract_fallback_bash() {
        let text = "Running test:\n```bash\ncargo check --workspace\n```";
        let calls = AgentLoop::extract_fallback_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "bash");
        assert_eq!(calls[0].arguments["command"], "cargo check --workspace");

        let text_sh = "```sh\npwd\n```";
        let calls_sh = AgentLoop::extract_fallback_tool_calls(text_sh);
        assert_eq!(calls_sh.len(), 1);
        assert_eq!(calls_sh[0].name, "bash");
        assert_eq!(calls_sh[0].arguments["command"], "pwd");
    }

    #[test]
    fn test_extract_fallback_write() {
        let text =
            "Creating file:\n```write src/main.rs\nfn main() {\n    println!(\"Hello\");\n}\n```";
        let calls = AgentLoop::extract_fallback_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "write");
        assert_eq!(calls[0].arguments["path"], "src/main.rs");
        assert_eq!(
            calls[0].arguments["content"],
            "fn main() {\n    println!(\"Hello\");\n}"
        );

        let text_json = "```write\n{\"path\": \"foo.txt\", \"content\": \"hello bar\"}\n```";
        let calls_json = AgentLoop::extract_fallback_tool_calls(text_json);
        assert_eq!(calls_json.len(), 1);
        assert_eq!(calls_json[0].name, "write");
        assert_eq!(calls_json[0].arguments["path"], "foo.txt");
        assert_eq!(calls_json[0].arguments["content"], "hello bar");
    }

    #[test]
    fn test_extract_fallback_edit() {
        let text_delim = "```edit src/lib.rs\nold content\n====\nnew content\n```";
        let calls_delim = AgentLoop::extract_fallback_tool_calls(text_delim);
        assert_eq!(calls_delim.len(), 1);
        assert_eq!(calls_delim[0].name, "edit");
        assert_eq!(calls_delim[0].arguments["path"], "src/lib.rs");
        assert_eq!(calls_delim[0].arguments["target"], "old content");
        assert_eq!(calls_delim[0].arguments["replacement"], "new content");

        let text_json = "```edit\n{\"path\": \"src/lib.rs\", \"target\": \"old\", \"replacement\": \"new\"}\n```";
        let calls_json = AgentLoop::extract_fallback_tool_calls(text_json);
        assert_eq!(calls_json.len(), 1);
        assert_eq!(calls_json[0].name, "edit");
        assert_eq!(calls_json[0].arguments["path"], "src/lib.rs");
        assert_eq!(calls_json[0].arguments["target"], "old");
        assert_eq!(calls_json[0].arguments["replacement"], "new");
    }

    #[test]
    fn test_extract_fallback_read() {
        let text_simple = "```read src/lib.rs\n```";
        let calls_simple = AgentLoop::extract_fallback_tool_calls(text_simple);
        assert_eq!(calls_simple.len(), 1);
        assert_eq!(calls_simple[0].name, "read");
        assert_eq!(calls_simple[0].arguments["path"], "src/lib.rs");

        let text_slice = "```read src/lib.rs 10 25\n```";
        let calls_slice = AgentLoop::extract_fallback_tool_calls(text_slice);
        assert_eq!(calls_slice.len(), 1);
        assert_eq!(calls_slice[0].name, "read");
        assert_eq!(calls_slice[0].arguments["path"], "src/lib.rs");
        assert_eq!(calls_slice[0].arguments["start_line"], 10);
        assert_eq!(calls_slice[0].arguments["end_line"], 25);

        let text_json =
            "```read\n{\"path\": \"src/lib.rs\", \"start_line\": 1, \"end_line\": 50}\n```";
        let calls_json = AgentLoop::extract_fallback_tool_calls(text_json);
        assert_eq!(calls_json.len(), 1);
        assert_eq!(calls_json[0].name, "read");
        assert_eq!(calls_json[0].arguments["path"], "src/lib.rs");
        assert_eq!(calls_json[0].arguments["start_line"], 1);
        assert_eq!(calls_json[0].arguments["end_line"], 50);
    }

    #[test]
    fn test_extract_fallback_multiple_and_non_tools() {
        let text = r#"
Here is some Rust code:
```rust
fn hello() {}
```

Now let's write and run:
```write test.txt
hello world
```

```bash
cat test.txt
```
"#;
        let calls = AgentLoop::extract_fallback_tool_calls(text);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "write");
        assert_eq!(calls[0].arguments["path"], "test.txt");
        assert_eq!(calls[1].name, "bash");
        assert_eq!(calls[1].arguments["command"], "cat test.txt");
    }

    async fn start_mock_sse_server(
        responses: Vec<String>,
    ) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://{}", addr);

        let handle = tokio::spawn(async move {
            for sse_body in responses {
                if let Ok((mut socket, _)) = listener.accept().await {
                    let mut buf = [0u8; 4096];
                    let _ = socket.read(&mut buf).await;

                    let http_response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        sse_body.len(),
                        sse_body
                    );
                    let _ = socket.write_all(http_response.as_bytes()).await;
                    let _ = socket.flush().await;
                }
            }
        });

        (base_url, handle)
    }

    #[tokio::test]
    async fn test_streaming_event_emission() {
        let sse_response = "data: {\"choices\":[{\"delta\":{\"content\":\"Hello \"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"world!\"}}]}\n\ndata: [DONE]\n\n".to_string();

        let (base_url, server_handle) = start_mock_sse_server(vec![sse_response]).await;

        let model_config = ModelConfig {
            provider: "openai".to_string(),
            model_id: "test-model".to_string(),
            api_key: "test-key".to_string(),
            base_url: Some(base_url),
            context_window: 128_000,
            max_output: 8_192,
        };

        let mut agent_loop = AgentLoop::new(model_config);
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        let res = agent_loop
            .run_turn("Hi", move |evt| {
                events_clone.lock().unwrap().push(evt);
            })
            .await
            .unwrap();

        assert_eq!(res, "Hello world!");

        let captured = events.lock().unwrap().clone();
        let streaming_chunks: Vec<String> = captured
            .iter()
            .filter_map(|e| match e {
                TurnEvent::ModelStreaming { chunk } => Some(chunk.clone()),
                _ => None,
            })
            .collect();

        assert_eq!(streaming_chunks, vec!["Hello ", "world!"]);
        assert!(
            captured
                .iter()
                .any(|e| matches!(e, TurnEvent::ContextPrepared { .. }))
        );
        assert!(
            captured
                .iter()
                .any(|e| matches!(e, TurnEvent::TurnCompleted { .. }))
        );

        let _ = server_handle.await;
    }

    #[tokio::test]
    async fn test_dual_tool_native_protocol_execution() {
        let tmp = tempfile::tempdir().unwrap();
        let test_file = tmp.path().join("native_out.txt");
        let test_file_str = test_file.to_str().unwrap().to_string();

        // Turn 1: LLM returns native tool call to write
        let turn1_json = serde_json::json!({
            "choices": [{
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "id": "call-write-1",
                        "function": {
                            "name": "write",
                            "arguments": serde_json::json!({
                                "path": test_file_str,
                                "content": "native tool content"
                            }).to_string()
                        }
                    }]
                }
            }]
        });
        let turn1_sse = format!(
            "data: {}\n\ndata: [DONE]\n\n",
            serde_json::to_string(&turn1_json).unwrap()
        );

        // Turn 2: LLM provides final answer
        let turn2_sse = "data: {\"choices\":[{\"delta\":{\"content\":\"File written successfully.\"}}]}\n\ndata: [DONE]\n\n".to_string();

        let (base_url, server_handle) = start_mock_sse_server(vec![turn1_sse, turn2_sse]).await;

        let model_config = ModelConfig {
            provider: "openai".to_string(),
            model_id: "test-model".to_string(),
            api_key: "test-key".to_string(),
            base_url: Some(base_url),
            context_window: 128_000,
            max_output: 8_192,
        };

        let mut agent_loop = AgentLoop::new(model_config);
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        let res = agent_loop
            .run_turn("Create native_out.txt", move |evt| {
                events_clone.lock().unwrap().push(evt);
            })
            .await
            .unwrap();

        assert_eq!(res, "File written successfully.");
        assert_eq!(
            fs::read_to_string(&test_file).unwrap(),
            "native tool content"
        );

        let captured = events.lock().unwrap().clone();
        assert!(captured.iter().any(|e| match e {
            TurnEvent::ToolExecuting { tool_name, .. } => tool_name == "write",
            _ => false,
        }));
        assert!(captured.iter().any(|e| match e {
            TurnEvent::ToolCompleted {
                tool_name,
                is_error,
            } => tool_name == "write" && !*is_error,
            _ => false,
        }));

        let _ = server_handle.await;
    }

    #[tokio::test]
    async fn test_dual_tool_markdown_fallback_execution_write_and_bash() {
        let tmp = tempfile::tempdir().unwrap();
        let test_file = tmp.path().join("md_out.txt");
        let test_file_str = test_file.to_str().unwrap().to_string();

        // Turn 1: LLM returns markdown fallback tool invocation
        let turn1_content = format!(
            "I will write the file:\n```write {}\nmarkdown file content\n```",
            test_file_str
        );
        let turn1_sse = format!(
            "data: {{\"choices\":[{{\"delta\":{{\"content\":{}}}}}]}}\n\ndata: [DONE]\n\n",
            serde_json::to_string(&turn1_content).unwrap()
        );

        // Turn 2: LLM provides final answer
        let turn2_sse = "data: {\"choices\":[{\"delta\":{\"content\":\"Finished writing via markdown fallback.\"}}]}\n\ndata: [DONE]\n\n".to_string();

        let (base_url, server_handle) = start_mock_sse_server(vec![turn1_sse, turn2_sse]).await;

        let model_config = ModelConfig {
            provider: "openai".to_string(),
            model_id: "test-model".to_string(),
            api_key: "test-key".to_string(),
            base_url: Some(base_url),
            context_window: 128_000,
            max_output: 8_192,
        };

        let mut agent_loop = AgentLoop::new(model_config);
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        let res = agent_loop
            .run_turn("Write markdown file", move |evt| {
                events_clone.lock().unwrap().push(evt);
            })
            .await
            .unwrap();

        assert_eq!(res, "Finished writing via markdown fallback.");
        assert_eq!(
            fs::read_to_string(&test_file).unwrap(),
            "markdown file content"
        );

        let captured = events.lock().unwrap().clone();
        assert!(captured.iter().any(|e| match e {
            TurnEvent::ToolExecuting { tool_name, .. } => tool_name == "write",
            _ => false,
        }));
        assert!(captured.iter().any(|e| match e {
            TurnEvent::ToolCompleted {
                tool_name,
                is_error,
            } => tool_name == "write" && !*is_error,
            _ => false,
        }));

        let _ = server_handle.await;
    }

    #[tokio::test]
    async fn test_dual_tool_markdown_fallback_execution_edit_and_read() {
        let tmp = tempfile::tempdir().unwrap();
        let test_file = tmp.path().join("edit_read.txt");
        let test_file_str = test_file.to_str().unwrap().to_string();

        fs::write(&test_file, "Initial text for editing").unwrap();

        // Turn 1: LLM returns edit fallback
        let turn1_content = format!(
            "```edit {}\nInitial text\n====\nUpdated text\n```",
            test_file_str
        );
        let turn1_sse = format!(
            "data: {{\"choices\":[{{\"delta\":{{\"content\":{}}}}}]}}\n\ndata: [DONE]\n\n",
            serde_json::to_string(&turn1_content).unwrap()
        );

        // Turn 2: LLM returns read fallback
        let turn2_content = format!("```read {}\n```", test_file_str);
        let turn2_sse = format!(
            "data: {{\"choices\":[{{\"delta\":{{\"content\":{}}}}}]}}\n\ndata: [DONE]\n\n",
            serde_json::to_string(&turn2_content).unwrap()
        );

        // Turn 3: LLM final answer
        let turn3_sse = "data: {\"choices\":[{\"delta\":{\"content\":\"Edit and read completed.\"}}]}\n\ndata: [DONE]\n\n".to_string();

        let (base_url, server_handle) =
            start_mock_sse_server(vec![turn1_sse, turn2_sse, turn3_sse]).await;

        let model_config = ModelConfig {
            provider: "openai".to_string(),
            model_id: "test-model".to_string(),
            api_key: "test-key".to_string(),
            base_url: Some(base_url),
            context_window: 128_000,
            max_output: 8_192,
        };

        let mut agent_loop = AgentLoop::new(model_config);
        let res = agent_loop.run_turn("Edit and read", |_| {}).await.unwrap();

        assert_eq!(res, "Edit and read completed.");
        assert_eq!(
            fs::read_to_string(&test_file).unwrap(),
            "Updated text for editing"
        );

        let _ = server_handle.await;
    }

    #[test]
    fn test_tokenize_args_quoted_spaces() {
        let tokens = AgentLoop::tokenize_args(r#""fn main()" "src/my project/" 10 20"#);
        assert_eq!(
            tokens,
            vec![
                "fn main()".to_string(),
                "src/my project/".to_string(),
                "10".to_string(),
                "20".to_string(),
            ]
        );

        let single_quoted = AgentLoop::tokenize_args(r#"'grep pattern' 'path with space'"#);
        assert_eq!(
            single_quoted,
            vec!["grep pattern".to_string(), "path with space".to_string()]
        );
    }

    #[test]
    fn test_extract_fallback_quoted_grep_read() {
        let text = r#"
```grep "fn main()" "src/my folder/"
```

```read "path with spaces/test.rs" 5 15
```
"#;
        let calls = AgentLoop::extract_fallback_tool_calls(text);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "grep");
        assert_eq!(calls[0].arguments["pattern"], "fn main()");
        assert_eq!(calls[0].arguments["path"], "src/my folder/");

        assert_eq!(calls[1].name, "read");
        assert_eq!(calls[1].arguments["path"], "path with spaces/test.rs");
        assert_eq!(calls[1].arguments["start_line"], 5);
        assert_eq!(calls[1].arguments["end_line"], 15);
    }

    #[tokio::test]
    async fn test_context_compaction_triggering() {
        let model_config = ModelConfig {
            provider: "openai".to_string(),
            model_id: "test-model".to_string(),
            api_key: "test-key".to_string(),
            base_url: None,
            context_window: 128_000,
            max_output: 8_192,
        };

        let mut agent_loop = AgentLoop::new(model_config);
        agent_loop.max_context_tokens = 200; // Small limit to trigger compaction

        // Populate history with several turns
        for i in 1..=8 {
            agent_loop.session_tree.append_child(
                Role::User,
                format!(
                    "User turn {} with long contextual instructions and detailed tasks",
                    i
                ),
            );
            agent_loop.session_tree.append_child(
                Role::Assistant,
                format!(
                    "Assistant response {} detailing executed changes and completed steps",
                    i
                ),
            );
        }

        let mut compacted = false;
        let res = agent_loop
            .compact_history_if_needed(&mut |evt| {
                if let TurnEvent::ContextCompacted {
                    old_turns,
                    new_summary_len,
                } = evt
                {
                    assert!(old_turns > 0);
                    assert!(new_summary_len > 0);
                    compacted = true;
                }
            })
            .await;

        assert!(res.is_ok());
        assert!(compacted);
        let history = agent_loop.session_tree.get_active_branch_history();
        assert!(history.iter().any(|n| n.role == Role::System && n.content.contains("Context Compaction Summary")));
    }

    #[test]
    fn test_openai_tool_call_id_in_conversation_messages() {
        let model_config = ModelConfig {
            provider: "openai".to_string(),
            model_id: "test-model".to_string(),
            api_key: "test-key".to_string(),
            base_url: None,
            context_window: 128_000,
            max_output: 8_192,
        };

        let mut agent_loop = AgentLoop::new(model_config);
        agent_loop
            .session_tree
            .append_child(Role::User, "Run check".to_string());

        let tool_call_json = serde_json::json!([{
            "id": "call_123",
            "type": "function",
            "function": {
                "name": "bash",
                "arguments": "{\"command\": \"cargo check\"}"
            }
        }]);

        agent_loop.session_tree.append_child_with_metadata(
            Role::Assistant,
            "".to_string(),
            None,
            None,
            Some(tool_call_json.clone()),
        );

        agent_loop.session_tree.append_child_with_metadata(
            Role::Tool,
            "Finished successfully".to_string(),
            Some("call_123".to_string()),
            Some("bash".to_string()),
            None,
        );

        let messages = agent_loop.build_conversation_messages();
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].tool_calls, Some(tool_call_json));
        assert_eq!(messages[2].role, "tool");
        assert_eq!(messages[2].tool_call_id, Some("call_123".to_string()));
        assert_eq!(messages[2].name, Some("bash".to_string()));
        assert_eq!(messages[2].content, "Finished successfully");
    }

    #[test]
    fn test_extract_fallback_grep_and_find_in_body() {
        let text = r#"
Here is the search:
```grep
main_function
```

And find files:
```find
*.rs
```
"#;
        let calls = AgentLoop::extract_fallback_tool_calls(text);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "grep");
        assert_eq!(calls[0].arguments["pattern"], "main_function");
        assert_eq!(calls[1].name, "find");
        assert_eq!(calls[1].arguments["pattern"], "*.rs");
    }

    #[test]
    fn test_extract_fallback_subagents() {
        let text = r#"
```invoke_subagent CodeReviewer
Please review the changes in src/lib.rs
```

```manage_subagents status 550e8400-e29b-41d4-a716-446655440000
```

```manage_subagents list
```
"#;
        let calls = AgentLoop::extract_fallback_tool_calls(text);
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0].name, "invoke_subagent");
        assert_eq!(calls[0].arguments["name"], "CodeReviewer");
        assert_eq!(
            calls[0].arguments["task"],
            "Please review the changes in src/lib.rs"
        );
        assert_eq!(calls[1].name, "manage_subagents");
        assert_eq!(calls[1].arguments["action"], "status");
        assert_eq!(
            calls[1].arguments["id"],
            "550e8400-e29b-41d4-a716-446655440000"
        );
        assert_eq!(calls[2].name, "manage_subagents");
        assert_eq!(calls[2].arguments["action"], "list");
    }

    #[test]
    fn test_extract_fallback_crew_tools() {
        let text = r#"
```crew_dispatch ship
Implement authentication wizard in pi-tui
```

```crew_status reconcile
```

```crew_merge abc12345 main
```
"#;
        let calls = AgentLoop::extract_fallback_tool_calls(text);
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0].name, "crew_dispatch");
        assert_eq!(calls[0].arguments["shape"], "ship");
        assert_eq!(
            calls[0].arguments["task"],
            "Implement authentication wizard in pi-tui"
        );
        assert_eq!(calls[1].name, "crew_status");
        assert_eq!(calls[1].arguments["action"], "reconcile");
        assert_eq!(calls[2].name, "crew_merge");
        assert_eq!(calls[2].arguments["task_id"], "abc12345");
        assert_eq!(calls[2].arguments["target_branch"], "main");
    }

    #[tokio::test]
    async fn test_allowed_tools_filtering_in_agent_loop() {
        let tmp = tempfile::tempdir().unwrap();
        let test_file = tmp.path().join("allowed_test.txt");
        let test_file_str = test_file.to_str().unwrap().to_string();

        // Model attempts to call write (allowed) and bash (disallowed)
        let turn1_json = serde_json::json!({
            "choices": [{
                "delta": {
                    "tool_calls": [
                        {
                            "index": 0,
                            "id": "call-write-1",
                            "function": {
                                "name": "write",
                                "arguments": serde_json::json!({
                                    "path": test_file_str,
                                    "content": "allowed write"
                                }).to_string()
                            }
                        },
                        {
                            "index": 1,
                            "id": "call-bash-1",
                            "function": {
                                "name": "bash",
                                "arguments": serde_json::json!({
                                    "command": "rm -rf /"
                                }).to_string()
                            }
                        }
                    ]
                }
            }]
        });
        let turn1_sse = format!(
            "data: {}\n\ndata: [DONE]\n\n",
            serde_json::to_string(&turn1_json).unwrap()
        );
        let turn2_sse = "data: {\"choices\":[{\"delta\":{\"content\":\"Completed allowed tool only.\"}}]}\n\ndata: [DONE]\n\n".to_string();

        let (base_url, server_handle) = start_mock_sse_server(vec![turn1_sse, turn2_sse]).await;

        let model_config = ModelConfig {
            provider: "openai".to_string(),
            model_id: "test-model".to_string(),
            api_key: "test-key".to_string(),
            base_url: Some(base_url),
            context_window: 128_000,
            max_output: 8_192,
        };

        let mut agent_loop = AgentLoop::new(model_config)
            .with_allowed_tools(vec!["write".to_string(), "read".to_string()]);

        let events = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        let res = agent_loop
            .run_turn("Do tasks", move |evt| {
                events_clone.lock().unwrap().push(evt);
            })
            .await
            .unwrap();

        assert_eq!(res, "Completed allowed tool only.");
        assert_eq!(fs::read_to_string(&test_file).unwrap(), "allowed write");

        // Verify that bash was NEVER executed
        let captured = events.lock().unwrap().clone();
        let executed_tools: Vec<String> = captured
            .iter()
            .filter_map(|e| match e {
                TurnEvent::ToolExecuting { tool_name, .. } => Some(tool_name.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(executed_tools, vec!["write".to_string()]);

        let _ = server_handle.await;
    }

    #[tokio::test]
    async fn test_double_compaction_cumulative_summary() {
        let model_config = ModelConfig {
            provider: "openai".to_string(),
            model_id: "test-model".to_string(),
            api_key: "test-key".to_string(),
            base_url: None,
            context_window: 128_000,
            max_output: 8_192,
        };

        let mut agent_loop = AgentLoop::new(model_config);
        agent_loop.max_context_tokens = 150; // Very small to trigger repeated compactions

        // Initial turns
        for i in 1..=6 {
            agent_loop.session_tree.append_child(
                Role::User,
                format!("User message {} with significant context", i),
            );
            agent_loop.session_tree.append_child(
                Role::Assistant,
                format!("Assistant reply {} summarizing operations", i),
            );
        }

        // First compaction
        let _ = agent_loop.compact_history_if_needed(&mut |_| {}).await;
        let history1 = agent_loop.session_tree.get_active_branch_history();
        assert!(history1.iter().any(|n| n.role == Role::System && n.content.contains("Context Compaction Summary")));

        // Add more turns
        for i in 7..=12 {
            agent_loop.session_tree.append_child(
                Role::User,
                format!("User message {} with further instructions", i),
            );
            agent_loop.session_tree.append_child(
                Role::Assistant,
                format!("Assistant reply {} confirming task", i),
            );
        }

        // Second compaction
        let mut second_compacted = false;
        let _ = agent_loop
            .compact_history_if_needed(&mut |evt| {
                if let TurnEvent::ContextCompacted { .. } = evt {
                    second_compacted = true;
                }
            })
            .await;

        assert!(second_compacted);
        let history2 = agent_loop.session_tree.get_active_branch_history();
        assert!(history2.iter().any(|n| n.role == Role::System && n.content.contains("Context Compaction Summary")));
    }

    #[test]
    fn test_tokenize_args_escape_and_nested() {
        let tokens = AgentLoop::tokenize_args(r#"path\ with\ spaces "arg 2" 'arg 3'"#);
        assert_eq!(tokens, vec!["path with spaces", "arg 2", "arg 3"]);
    }

    #[test]
    fn test_system_prompt_engine_with_vault_hindsight_injection() {
        let vault = TauVault::open_in_memory().unwrap();
        vault
            .record_counter_rule(
                "slice_safety",
                "&s[..len] byte slicing",
                "floor_char_boundary",
            )
            .unwrap();

        let engine = SystemPromptEngine {
            base_prompt: "Pi system".to_string(),
            agents_md: None,
            skill_registry: SkillRegistry::default(),
            vault,
        };

        let full = engine.build_full_prompt();
        assert!(full.contains("[Hindsight Memory & Rules]"));
        assert!(full.contains("[Counter-Rule] (slice_safety)"));
        assert!(full.contains("floor_char_boundary"));
    }

    #[tokio::test]
    async fn test_agent_loop_reflexion_on_failed_tool() {
        // Mock tool call that fails (e.g. read non-existent file)
        let turn1_json = serde_json::json!({
            "choices": [{
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "id": "call-read-fail-1",
                        "function": {
                            "name": "read",
                            "arguments": serde_json::json!({
                                "path": "non_existent_file_xyz_123.rs"
                            }).to_string()
                        }
                    }]
                }
            }]
        });
        let turn1_sse = format!(
            "data: {}\n\ndata: [DONE]\n\n",
            serde_json::to_string(&turn1_json).unwrap()
        );
        let turn2_sse = "data: {\"choices\":[{\"delta\":{\"content\":\"Understood, file was missing.\"}}]}\n\ndata: [DONE]\n\n".to_string();

        let (base_url, server_handle) = start_mock_sse_server(vec![turn1_sse, turn2_sse]).await;

        let model_config = ModelConfig {
            provider: "openai".to_string(),
            model_id: "test-model".to_string(),
            api_key: "test-key".to_string(),
            base_url: Some(base_url),
            context_window: 128_000,
            max_output: 8_192,
        };

        let mut agent_loop = AgentLoop::new(model_config);
        // Use an isolated in-memory vault for this loop test
        let in_mem_vault = TauVault::open_in_memory().unwrap();
        agent_loop.system_engine.vault = in_mem_vault.clone();

        let res = agent_loop
            .run_turn("Read missing file", |_| {})
            .await
            .unwrap();

        assert_eq!(res, "Understood, file was missing.");

        // Verify ReflexionEngine recorded the failure counter-rule in the vault
        let active = in_mem_vault.list_active_memories(10).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].topic, "tool_failure:read");
        assert!(
            active[0]
                .correct_pattern
                .as_ref()
                .unwrap()
                .contains("find or ls")
        );

        let _ = server_handle.await;
    }

    #[test]
    fn test_agent_loop_plan_mode_methods() {
        let model_config = ModelConfig {
            provider: "openai".to_string(),
            model_id: "test-model".to_string(),
            api_key: "test-key".to_string(),
            base_url: None,
            context_window: 128_000,
            max_output: 8_192,
        };

        let mut plan = ExecutionPlan::new("p1", "Add login auth");
        let t1 = PlanTask::new("t1", "Design API", "Design tokens");
        let t2 = PlanTask::new("t2", "Implement endpoint", "Write route")
            .with_dependencies(vec!["t1".to_string()]);
        plan.add_task(t1);
        plan.add_task(t2);

        let mut agent_loop = AgentLoop::new(model_config).with_execution_plan(plan);
        assert!(agent_loop.get_execution_plan().is_some());
        assert_eq!(agent_loop.get_execution_plan().unwrap().tasks.len(), 2);

        let md = agent_loop.format_plan_markdown().unwrap();
        assert!(md.contains("### Plan: Add login auth"));
        assert!(md.contains("Design API"));
        assert!(md.contains("Implement endpoint"));

        if let Some(plan_mut) = agent_loop.get_execution_plan_mut() {
            plan_mut.tasks[0].status = TaskStatus::Completed {
                duration_ms: 100,
                summary: "Tokens designed".to_string(),
            };
        }

        let updated_md = agent_loop.format_plan_markdown().unwrap();
        assert!(updated_md.contains("[✔] 1. **Design API**"));
        assert!(updated_md.contains("[ ] 2. **Implement endpoint**"));
    }

    #[test]
    fn test_agent_loop_crystallize_active_session() {
        let model_config = ModelConfig {
            provider: "openai".to_string(),
            model_id: "test-model".to_string(),
            api_key: "test-key".to_string(),
            base_url: None,
            context_window: 128_000,
            max_output: 8_192,
        };

        let mut agent_loop = AgentLoop::new(model_config);
        agent_loop
            .session_tree
            .append_child(Role::User, "Configure Rust release profiles".to_string());
        agent_loop.session_tree.append_child_with_metadata(
            Role::Assistant,
            "Editing Cargo.toml".to_string(),
            None,
            None,
            Some(serde_json::json!([
                {
                    "id": "c1",
                    "type": "function",
                    "function": {
                        "name": "bash",
                        "arguments": "{\"command\": \"cargo test --release\"}"
                    }
                }
            ])),
        );

        let skill_path = agent_loop
            .crystallize_active_session(
                "Rust Release Optimizer",
                "Optimizes release profile flags and builds",
            )
            .unwrap();

        assert!(skill_path.exists());
        assert!(
            agent_loop
                .system_engine
                .skill_registry
                .get_skill("rust-release-optimizer")
                .is_some()
        );
    }
}
