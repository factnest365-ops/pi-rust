# AGENTS.md — Master Agent Operating Guidelines & Invariants

Welcome to **pi-rust** (`pi-rs`), a high-performance, 100% pure Rust port of Mario Zechner's Pi Coding Agent ([pi.dev](https://pi.dev) / [`earendil-works/pi`](https://github.com/earendil-works/pi)).

This document is the **authoritative reference manual** for all AI coding agents working in this repository. Any agent (Antigravity, Claude Code, Cursor, Copilot, Windsurf, Cline, Roo, Aider) operating here must adhere strictly to these principles, invariants, schemas, and verification protocols.

---

## 1. Architectural Blueprint & Crate Dependency Graph

The workspace is strictly partitioned into 7 focused Cargo crates under `crates/`. There is **no monolithic root `src/`**.

```
                                +-----------------------------------+
                                |              pi-cli               |
                                |   (Entry Binary & CLI Dispatcher) |
                                +-----------------+-----------------+
                                                  |
         +----------------------------------------+----------------------------------------+
         |                                        |                                        |
         v                                        v                                        v
+----------------+                       +----------------+                       +----------------+
|     pi-tui     |                       |    pi-core     |                       |     pi-rpc     |
| (Ratatui & UI) |                       | (Agent Engine) |                       |  (JSON-RPC 2.0)|
+-------+--------+                       +---+---+----+---+                       +--------+-------+
        |                                    |   |    |                                    |
        +------------------+-----------------+   |    +------------------+-----------------+
                           |                     |                       |
                           v                     |                       v
                  +-----------------+            |              +-----------------+
                  |  pi-providers   |            |              |   pi-session    |
                  | (Multi-LLM SSE) |            |              |  (DAG History & |
                  +-----------------+            |              |  JSONL Storage) |
                                                 |              +-----------------+
                                                 v
                                        +-----------------+
                                        |    pi-tools     |
                                        | (Native Tools & |
                                        |  MCP Discovery) |
                                        +-----------------+
```

### Crate Parity Mapping to Upstream Pi (`earendil-works/pi`)

| Rust Crate (`pi-rust`) | Upstream TypeScript Package (`earendil-works/pi`) | Primary Responsibility |
| :--- | :--- | :--- |
| [`crates/pi-cli`](file:///Users/bhavy/pi-rust/crates/pi-cli) | `packages/client` | CLI argument parsing (`clap`), login wizard, `--rpc`, `--print`, `--model`. |
| [`crates/pi-core`](file:///Users/bhavy/pi-rust/crates/pi-core) | `packages/coding-agent` & `packages/agent` | Agent turn loop, Dual Tool dispatch, Context Compaction, System Prompt Engine. |
| [`crates/pi-providers`](file:///Users/bhavy/pi-rust/crates/pi-providers) | `packages/ai` | 33+ Multi-LLM provider client, SSE streaming, TokenProfiler, AuthResolver. |
| [`crates/pi-session`](file:///Users/bhavy/pi-rust/crates/pi-session) | `packages/agent` & `session-backends` | Session DAG tree, node ID assignment, branch rewinds, JSONL disk persistence. |
| [`crates/pi-tools`](file:///Users/bhavy/pi-rust/crates/pi-tools) | `packages/coding-agent/src/tools/*` | Safe tool executor (`read`, `write`, `edit`, `bash`, `grep`, `find`, `ls`, `web_fetch`, `web_search`, `git`, `github`, `lsp`, `ast`, `mcp`). |
| [`crates/pi-tui`](file:///Users/bhavy/pi-rust/crates/pi-tui) | `packages/tui` | Ratatui terminal UI, interactive overlays (Model Picker, Provider Picker, Session Tree, Auth Wizard), syntax highlighting, ASCII/Unicode Mermaid renderer. |
| [`crates/pi-rpc`](file:///Users/bhavy/pi-rust/crates/pi-rpc) | `packages/protocol` & `packages/server` | Bi-directional JSON-RPC 2.0 daemon over stdin/stdout with ordered MPSC event streaming. |

---

## 2. Strict Architectural Invariants & Rules

### Invariant 1: Dependency Flow & Decoupling
- `pi-cli` depends on all crates.
- `pi-tui` depends on `pi-core`, `pi-providers`, `pi-session`.
- `pi-rpc` depends on `pi-core`, `pi-providers`, `pi-session`, `pi-tools`.
- `pi-core` depends on `pi-providers`, `pi-session`, `pi-tools`.
- **Leaf Crates Must Remain Decoupled:** `pi-providers`, `pi-session`, and `pi-tools` must never depend on each other or on `pi-core`/`pi-tui`/`pi-rpc`.

### Invariant 2: Session DAG Message Causality & Node Metadata
- **Strict DAG Causality:** Always append `Role::Assistant` (containing tool call metadata) to `SessionTree` *before* executing tools and appending subsequent `Role::Tool` outputs.
- **Node Metadata:** When recording tool invocations and results in [`SessionNode`](file:///Users/bhavy/pi-rust/crates/pi-session/src/lib.rs), always populate `tool_call_id`, `tool_name`, and `tool_calls`.

### Invariant 3: Provider Protocol Compliance (OpenAI & Anthropic)
- **OpenAI Native Tool Calling:** Every message with `role: "tool"` must transmit `"tool_call_id": "<id>"`, and `role: "assistant"` must transmit `"tool_calls": [...]`.
- **Anthropic Messages API:** Tool results must be serialized inside `user` role messages as `{"type": "tool_result", "tool_use_id": "...", "content": "..."}` blocks.
- **SSE Stream Splitting:** Accumulate bytes and process SSE deltas across packet fragments cleanly.

### Invariant 4: Dual Tool Protocol
All registered tools must be defined in both:
1. `ToolExecutor::tool_definitions()` in [`crates/pi-tools`](file:///Users/bhavy/pi-rust/crates/pi-tools/src/lib.rs) (JSON Schema for frontier models).
2. `AgentLoop::extract_fallback_tool_calls()` in [`crates/pi-core`](file:///Users/bhavy/pi-rust/crates/pi-core/src/lib.rs) (Markdown fenced-code block parsing for local/smaller models).

### Invariant 5: Subprocess Safety & Timeout Guarantees
- Shell and subprocess execution (`bash`, `git`, `github`, `lsp`, `mcp`) must be **asynchronous** (`tokio::process::Command`) with explicit timeouts (120s default).
- Always ensure `child.kill().await` and `child.wait().await` are invoked on timeouts or cancellation to prevent zombie process leaks.

### Invariant 6: Context Window Compaction Loop
- `TokenProfiler` monitors context capacity. When conversation tokens approach **80% of context limit**, `AgentLoop::compact_history_if_needed`:
  1. Locates turn cut boundaries aligned to a `Role::User` cut point.
  2. Generates a structured summary of the older turns.
  3. Creates a synthetic compaction branch in the session DAG and preserves active recent turns.
  4. Emits `TurnEvent::ContextCompacted`.

### Invariant 7: Stdout Hygiene in JSON-RPC Mode
- **Never emit human-readable logs to stdout in `--rpc` mode.** Stdout is strictly reserved for valid JSON-RPC frames.
- All operational, diagnostic, and debug messages must route to `eprintln!`.
- JSON-RPC notifications (`pi/streamingChunk`, `pi/toolExecuting`, etc.) must be serialized through an in-order MPSC channel before the final `RpcResponse` is written.

### Invariant 8: Non-Blocking UI Event Loop
- Never call synchronous blocking I/O or long `.await` network requests directly on the main Ratatui event thread.
- Always dispatch background tasks (e.g. `ModelCatalogLoader::fetch_all_models`) via Tokio tasks and communicate results back over `AgentTaskEvent` MPSC channels.

### Invariant 9: String Slicing UTF-8 Character Boundaries
- **Never** slice UTF-8 strings by raw byte indices (`&s[..len]`). Always use `s.floor_char_boundary(len)` to prevent runtime panics on multibyte Unicode characters, emojis, and HTML entities.

---

## 3. Native Tool Contracts & Schemas

| Tool Name | Action / Purpose | Key Parameters | Line Indexing / Safety Rule |
| :--- | :--- | :--- | :--- |
| **`read`** | View contents or line slices of a file | `path` (string), `start_line` (int, opt), `end_line` (int, opt) | **1-indexed lines**. Guard: `start_line >= 1` and `start_line <= end_line`. |
| **`write`** | Create or overwrite a file | `path` (string), `content` (string) | **Atomic directory creation**: Automatically invokes `fs::create_dir_all`. |
| **`edit`** | Surgical find-and-replace text edit | `path` (string), `target` / `oldText` (string), `replacement` / `newText` (string) | **Unambiguity Check**: Verifies `target` exists and occurs exactly once (`occurrences == 1`). Errors if duplicate matches are found. |
| **`bash`** | Execute shell commands in workspace | `command` (string), `cwd` (string, opt) | **Async timeout**: 120s timeout with process tree kill on timeout or interruption. |
| **`grep`** | Fast recursive pattern search | `pattern` (string), `path` (string, opt) | **Flag Safety**: Passes `-e` before pattern to prevent hyphen option injection. |
| **`find`** | Find files by pattern or glob | `pattern` (string), `path` (string, opt) | Recursively searches within path matching pattern. |
| **`ls`** | List directory contents | `path` (string, opt) | Lists file names, directory indicators, and file sizes in bytes. |
| **`web_fetch`** | Fetch URL and convert HTML to markdown | `url` (string) | Connection-pooled `reqwest` client; decodes entities and strips scripts/styles. |
| **`web_search`** | Query live search engine | `query` (string) | Queries search engine and extracts organic result links and snippets. |
| **`git`** | Execute git operations | `action` (`status`, `diff`, `log`, `commit`), `staged`, `file`, `count`, `message` | Parameterized `Command` execution without shell interpolation. |
| **`github`** | Inspect PRs and issues via `gh` CLI | `action` (`pr_list`, `pr_view`, `pr_diff`, `issue_list`, `issue_view`, `run_list`), `pr`, `issue`, `limit` | Parameterized CLI execution capturing structured output. |
| **`lsp`** | Query language server diagnostics & symbols | `action` (`diagnostics`, `symbols`, `definition`, `hover`), `path`, `symbol` | Dispatches language compiler checks and symbol search. |
| **`ast`** | Syntactically slice symbols & functions | `path` (string), `symbol` (string, opt) | Exact token boundary matching without line guessing. |
| **`mcp`** | Call Model Context Protocol server tools | Dynamic tool name and JSON parameters | Out-of-process stdio/HTTP MCP tool execution with process cleanup. |

---

## 4. Coding Conventions & Quality Gates

- **Edition**: Rust 2024 / latest stable Rust idioms.
- **Clippy Policy**: `cargo clippy --workspace --all-targets -- -D warnings` must produce **0 warnings**.
- **Error Handling**: Use `anyhow::Result` for application boundaries with actionable context.
- **Pattern Matching**: Prefer expressive pattern matching (`let ... else`, `if ... && let Ok(...)`) over deeply nested blocks.
- **Memory Safety**: 100% safe Rust. No `unsafe` blocks without documented safety proofs.

---

## 5. Verification Checklist for All Changes

Before submitting or completing any task:

```bash
# 1. Type check all workspace crates and targets
cargo check --workspace --all-targets

# 2. Strict Clippy check with zero warnings
cargo clippy --workspace --all-targets -- -D warnings

# 3. Complete unit and integration test suite
cargo test --workspace -- --nocapture
```

- [ ] All 64+ workspace tests pass with 100% success rate.
- [ ] Session message causality maintained (`User` $\to$ `Assistant` $\to$ `Tool`).
- [ ] OpenAI `tool_call_id` and Anthropic `tool_result` protocol compliance verified.
- [ ] String slicing uses `floor_char_boundary`.
- [ ] Subprocess execution uses async timeouts and kills child handles on timeout.
- [ ] JSON-RPC stdout is pure JSON-RPC (all logs route to `eprintln!`).
- [ ] No dead code, debug prints, or unhandled unwraps left in production code paths.
