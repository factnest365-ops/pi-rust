# SWARM_TASK_PROMPTS.md — Ready-to-Execute Agent Prompt Packages

This document contains **self-contained, copy-pasteable prompt packages** for autonomous AI coding agents (Antigravity, Claude Code, Cursor, Copilot, Cline, Roo, Aider, Windsurf) to execute and complete all remaining phases of `pi-rust` (`pi-rs`).

Each prompt includes:
1. **Context & Invariants**: Strict architectural boundaries and rules.
2. **Actionable Implementation Steps**: Exact files, structs, and methods to create/modify.
3. **Dual Protocol Compliance**: JSON schema and Markdown fallback registration.
4. **Verification Gates**: Commands and assertions the agent must verify before completion.

---

## Prompt Package 1: Subagent Delegation Primitives (`pi-core` & `pi-tools`)

```markdown
You are a senior Rust systems engineer implementing Subagent Delegation Primitives in `pi-rust`.

### Objective:
Implement `SubagentRunner` in `pi-core` and subagent tools (`invoke_subagent`, `manage_subagents`) in `pi-tools` to enable the primary agent loop to spawn, message, and monitor background subagents with isolated context windows.

### Architecture & Requirements:
1. **Subagent Definition & State Machine (`crates/pi-core/src/subagents.rs`)**:
   - Create `SubagentConfig` (name, model_override, system_prompt_override, allowed_tools).
   - Create `SubagentInstance` (id: Uuid, name: String, agent_loop: AgentLoop, status: SubagentStatus { Running, Idle, Finished(String), Errored(String) }).
   - Create `SubagentManager` (manages active child instances in a `HashMap<String, SubagentInstance>`).
2. **Tool Implementations (`crates/pi-tools/src/subagents.rs` & `crates/pi-tools/src/lib.rs`)**:
   - `invoke_subagent`: Arguments: `{"name": "...", "task": "...", "model": "...", "tools": [...]}`. Spawns Tokio background task running child `AgentLoop`.
   - `manage_subagents`: Arguments: `{"action": "list" | "kill" | "status", "id": "..."}`.
3. **Dual Tool Protocol**:
   - Add tool definitions to `ToolExecutor::tool_definitions()` in `crates/pi-tools/src/lib.rs`.
   - Add markdown fallback extractor to `AgentLoop::parse_markdown_block()` in `crates/pi-core/src/lib.rs` for ````invoke_subagent```` and ````manage_subagents````.
4. **DAG Session Integrity**:
   - Subagent executions should emit `Role::Assistant` and `Role::Tool` nodes with `tool_name: Some("invoke_subagent")` and child session references.

### Invariants:
- Leaf crates (`pi-tools`, `pi-session`, `pi-providers`) MUST NOT depend on `pi-core`. Define tool payload structs cleanly or bridge via callbacks/MPSC channels.
- Subagent processes must have cancellation tokens (`tokio_util::sync::CancellationToken`) to prevent background leaks.

### Quality Gates:
- Add unit tests in `crates/pi-core/src/subagents.rs` and `crates/pi-tools/src/lib.rs`.
- Verify:
  ```bash
  cargo check --workspace --all-targets
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace -- --nocapture
  ```
```

---

## Prompt Package 2: Git Worktree Workspace Isolation (`pi-tools`)

```markdown
You are a senior Rust systems engineer implementing Git Worktree Workspace Isolation for `pi-rust`.

### Objective:
Implement automated Git Worktree isolation so background subagents and experimental tasks can modify code in an isolated directory without polluting the primary working branch.

### Architecture & Requirements:
1. **Git Worktree Operations (`crates/pi-tools/src/git.rs`)**:
   - Implement `git_worktree_create(base_branch: &str, task_id: &str) -> Result<PathBuf>`:
     Runs `git worktree add -b pi-task-<task_id> .pi/worktrees/<task_id> <base_branch>`.
   - Implement `git_worktree_remove(task_id: &str, force: bool) -> Result<()>`:
     Runs `git worktree remove .pi/worktrees/<task_id>`.
   - Implement `git_worktree_merge(task_id: &str, target_branch: &str) -> Result<String>`:
     Merges completed worktree branch back into target branch with conflict diagnostics.
   - Implement `git_worktree_list() -> Result<Vec<WorktreeInfo>>`.
2. **Tool Integration (`crates/pi-tools/src/lib.rs`)**:
   - Extend `execute_git` to support actions: `worktree_add`, `worktree_remove`, `worktree_list`, `worktree_merge`.
   - Update `ToolExecutor::tool_definitions()` with JSON schemas for worktree actions.
   - Update markdown fallback parser in `crates/pi-core/src/lib.rs` for ````git worktree_...````.

### Invariants:
- All git commands must use parameterized `tokio::process::Command` (no shell interpolation).
- Always clean up temporary worktrees on errors or task aborts.
- Automatic creation of `.pi/worktrees` directory with `.gitignore` exclusion.

### Quality Gates:
- Add integration tests creating a temporary git repository, creating a worktree, committing changes, and verifying isolation.
- Verify:
  ```bash
  cargo check --workspace --all-targets
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace -- --nocapture
  ```
```

---

## Prompt Package 3: Trajectory Time-Travel & Deterministic Session Replay (`pi-session` & `pi-core`)

```markdown
You are a senior Rust systems engineer implementing Session Trajectory Time-Travel and Replay in `pi-rust`.

### Objective:
Implement deterministic trajectory playback, session branching diffs, and JSONL turn simulation in `pi-session` and `pi-core`.

### Architecture & Requirements:
1. **Trajectory Simulator & Exporter (`crates/pi-session/src/lib.rs`)**:
   - Implement `SessionTree::export_trajectory(&self, branch_node_id: Option<&str>) -> TrajectoryExport`:
     Extracts a linear, step-by-step transcript with timestamps, token usage, tool invocations, and tool outputs.
   - Implement `SessionTree::diff_branches(&self, node_a: &str, node_b: &str) -> BranchDiff`:
     Computes the lowest common ancestor (LCA) and returns divergent node sequences.
   - Implement `SessionTree::simulate_rewind_to(&self, target_node_id: &str) -> Vec<&SessionNode>`:
     Returns what the active history would become without mutating internal state.
2. **RPC & CLI Bridging**:
   - Add `pi/session/trajectory` and `pi/session/diff` RPC endpoints in `crates/pi-rpc/src/lib.rs`.
   - Add `--replay <session_file.jsonl>` mode in `crates/pi-cli/src/main.rs` to stream a stored session to stdout/TUI at original pacing.

### Invariants:
- Trajectory replay must be strictly deterministic and read-only.
- DAG traversal must use `HashSet` cycle detection.

### Quality Gates:
- Add unit tests in `crates/pi-session/src/lib.rs` verifying LCA computation, branch diffing, and export serialization.
- Verify:
  ```bash
  cargo check --workspace --all-targets
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace -- --nocapture
  ```
```

---

## Prompt Package 4: Interactive Terminal Diff Visualizer Overlay (`pi-tui`)

```markdown
You are a senior Rust systems engineer implementing an Interactive Diff Visualizer Overlay in `pi-tui`.

### Objective:
Implement an interactive side-by-side / unified terminal diff viewer popup overlay in `pi-tui` (using Ratatui) to inspect file modifications proposed by `write` or `edit` tools before execution.

### Architecture & Requirements:
1. **Diff Calculator & Formatter (`crates/pi-tui/src/diff_view.rs`)**:
   - Implement `DiffView::compute_unified_diff(old_content: &str, new_content: &str, file_path: &str) -> Vec<DiffLine>`.
   - Distinguish line types: `DiffLine::Addition(String)`, `DiffLine::Deletion(String)`, `DiffLine::Context(String)`, `DiffLine::Header(String)`.
2. **Ratatui Component Rendering (`crates/pi-tui/src/diff_view.rs`)**:
   - Render styled lines: Additions in `Color::Green` (`+`), Deletions in `Color::Red` (`-`), Headers in `Color::Cyan`.
   - Support scrolling via `Up` / `Down` / `PageUp` / `PageDown` / `Home` / `End`.
   - Support `[y] Accept` / `[n] Reject` / `[Esc] Close` keybindings.
3. **App State Machine Integration (`crates/pi-tui/src/lib.rs`)**:
   - Add `AppOverlay::DiffViewer(DiffViewState)` to `PiTuiApp`.
   - Add slash command `/diff <file>` or trigger on pending edits.

### Invariants:
- Never block the 25ms Ratatui UI event loop.
- Use `floor_char_boundary` for all Unicode text slicing in diff rendering.

### Quality Gates:
- Add unit tests for diff computation and line classification in `crates/pi-tui/src/diff_view.rs`.
- Verify:
  ```bash
  cargo check --workspace --all-targets
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace -- --nocapture
  ```
```

---

## Prompt Package 5: JSON-RPC 2.0 WebSockets & Batch Protocol (`pi-rpc`)

```markdown
You are a senior Rust systems engineer expanding `pi-rpc` with WebSocket transport and Batch Request processing.

### Objective:
Implement WebSockets transport and JSON-RPC 2.0 batch array request parsing (`[ {...}, {...} ]`) in `pi-rpc`.

### Architecture & Requirements:
1. **JSON-RPC 2.0 Batch Handling (`crates/pi-rpc/src/lib.rs`)**:
   - Support parsing single `RpcRequest` or batch `Vec<RpcRequest>`.
   - Process requests concurrently via `tokio::task::JoinSet` while maintaining response array ordering.
2. **WebSocket Transport Daemon (`crates/pi-rpc/src/ws.rs`)**:
   - Implement `RpcServer::run_websocket_server(bind_addr: &str)` using `tokio-tungstenite`.
   - Broadcast streaming notifications (`pi/streamingChunk`, `pi/toolExecuting`) to connected client sockets.
3. **CLI Integration (`crates/pi-cli/src/main.rs`)**:
   - Add `--rpc-ws [PORT]` flag (default port 8765) to launch the WebSocket RPC daemon alongside stdio.

### Invariants:
- Never write human logs to stdout when in `--rpc` or `--rpc-ws` mode.
- Maintain leaf crate decoupling.

### Quality Gates:
- Add unit tests for batch request serialization and WebSocket connection handshakes.
- Verify:
  ```bash
  cargo check --workspace --all-targets
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace -- --nocapture
  ```
```

---

## Master Swarm Dispatch Order

To complete the entire roadmap with a swarm of agents, dispatch the packages in this exact dependency order:

1. **Agent 1** $\to$ **Prompt Package 2** (`Git Worktree Workspace Isolation` in `pi-tools`)
2. **Agent 2** $\to$ **Prompt Package 3** (`Trajectory Time-Travel & Replay` in `pi-session`)
3. **Agent 3** $\to$ **Prompt Package 1** (`Subagent Delegation Primitives` in `pi-core` & `pi-tools`)
4. **Agent 4** $\to$ **Prompt Package 4** (`Terminal Diff Visualizer Overlay` in `pi-tui`)
5. **Agent 5** $\to$ **Prompt Package 5** (`JSON-RPC WebSockets & Batch Protocol` in `pi-rpc`)
