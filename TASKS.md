# TASKS.md — Project Task Tracker & Execution Board

## Current Status Overview
- **Project**: `pi-rust` (`pi-rs` / `tau`) — 100% Pure Rust Port of Mario Zechner's Pi Coding Agent ([pi.dev](https://pi.dev) / [`earendil-works/pi`](https://github.com/earendil-works/pi))
- **Target Parity**: Complete functional, aesthetic, protocol, and architectural parity with `@earendil-works/pi`.
- **Compiler/Lint Status**: Zero warnings, 100% Clippy clean (`cargo clippy --workspace --all-targets -- -D warnings`), 100% test pass rate (197/197 tests passing) across all 7 workspace crates.

---

## 1. Recently Completed Tasks (Sprint Hardening, Bug Remediation & TUI Performance) ✅

- [x] **Full Codebase Audit & Critical Bug Remediation (All 7 Crates)**:
  - [x] **`pi-session` DAG Role Serialization**: Added lowercase serde deserialization and case-insensitive loading in JSONL persistence to ensure `user` and `assistant` roles are preserved rather than converting to `System`.
  - [x] **`pi-session` DAG Node ID & Tree Reconstruction**: Upgraded to 12-hex collision-free IDs and added a full `children_ids` relationship reconstruction pass on JSONL load.
  - [x] **`pi-tools` Subprocess Deadlock & Zombie Elimination**: Replaced sequential pipe reads in `execute_bash_async` with concurrent `tokio::join!`, and ensured `child.wait().await` follows `child.kill().await` on command timeouts.
  - [x] **`pi-tools` String Slicing & Validation**: Fixed CRLF normalization and empty target check in `execute_edit`. Replaced lowercased UTF-8 byte slicing in `WebTool::html_to_markdown` with ASCII case-insensitive byte matching.
  - [x] **`pi-tools` MCP & Worktree Handling**: Propagated `isError: true` application errors in `McpManager`. Added disposable `pi-task-*` branch deletion during worktree removal and `git merge --abort` on conflict detection.
  - [x] **`pi-providers` Multi-Turn Tool Result Merging**: Fixed Anthropic Messages API consecutive `user` role bug when appending user messages following tool result JSON arrays; dynamically set Anthropic max tokens to 8192; reset SSE event state after consuming payloads.
  - [x] **`pi-core` Context Compaction & Protocol**: Preserved older compaction summary texts during successive compactions; fixed Herdr OSC 1337 Base64 encoding (`d29ya2luZw==`, `YmxvY2tlZA==`, etc.); converted FirstMate verification subprocesses to async `tokio::process::Command`.
  - [x] **`pi-tui` Zero-Lag Rendering & UTF-8 Safety**: Removed synchronous `std::process::Command` calls from the 25ms `draw()` closure, caching git info via non-blocking background tasks; added UTF-8 `floor_char_boundary` safety across all string slicing; fixed status bar padding calculation with Unicode character widths; added windowed pagination in autocomplete popup; added $O(N+M)$ prefix/suffix fallback in diff viewer.
  - [x] **`pi-rpc` & `pi-cli` Protocol Fidelity**: Accepted initial `--model` in `RpcServer::run_stdin_stdout_loop`; returned standard JSON-RPC 2.0 `-32700` (Parse error) and suppressed responses for notifications; cleaned up `--print` mode stdout hygiene; guarded onboarding wizard with interactive terminal check.

- [x] **OpenAI & Anthropic Native Tool Calling Protocol Compliance**:
  - [x] Extended `ChatMessage` and `SessionNode` with `tool_call_id`, `tool_name`, and structured `tool_calls`.
  - [x] Enforced OpenAI `tool_call_id` inclusion on `role: "tool"` messages to eliminate HTTP 400 rejections on multi-turn tool execution.
  - [x] Structured Anthropic `tool_result` content blocks inside user messages matching Messages API specification.
  - [x] Added unit tests for tool message schema fidelity (`test_openai_tool_call_id_in_conversation_messages`).

- [x] **Automated Context Window Compaction Loop**:
  - [x] Integrated `TokenProfiler` with automatic turn boundary cut-point discovery (`findTurnStartIndex`).
  - [x] Implemented `AgentLoop::compact_history_if_needed` to automatically synthesize summary branches when context reaches 80% threshold.
  - [x] Added automated unit tests verifying compaction triggering (`test_context_compaction_triggering`).

- [x] **Async Subprocess Safety & Process Lifecycle**:
  - [x] Converted `execute_bash` to async `tokio::process::Command` with 120s timeout and child process killing on timeout.
  - [x] Added working directory (`cwd`) support to `execute_bash`.
  - [x] Wrapped MCP `fetch_stdio_tools` and `execute_stdio_tool` in async evaluation blocks ensuring `child.kill().await` runs on timeouts/errors to eliminate zombie process leaks.
  - [x] Added unit tests for async bash execution (`test_bash_cwd_and_execution`).

- [x] **Ordered JSON-RPC Stream Delivery**:
  - [x] Replaced un-ordered `tokio::spawn` stdout writes in `RpcServer::run_stdin_stdout_loop` with an unbounded MPSC queue.
  - [x] Guaranteed strict FIFO ordering of all streaming notifications and complete flush before final response frame output.

- [x] **Non-Blocking TUI Event Loop & Tool Defenses**:
  - [x] Replaced synchronous `.await` in `/refresh` and `Ctrl+R` with non-blocking background Tokio tasks.
  - [x] Implemented substring ambiguity defense in `execute_edit` (errors when `occurrences > 1` with helpful diagnostic).
  - [x] Added `-e` flag protection to `execute_grep` to prevent flag injection on hyphen patterns.
  - [x] Fixed wildcard default fallback in `AuthResolver::save_key` to preserve custom provider namespaces in `~/.pi/config.json`.

- [x] **First Mate & Herdr Swarm Coordination**:
  - [x] Implemented `HerdrProtocol` with OSC escape sequence emission (`working`, `blocked`, `done`, `idle`) and environment auto-detection (`HERDR_SESSION`, `TMUX`, `ZELLIJ`, `FIRSTMATE_HOME`).
  - [x] Implemented `FirstMateDistro` orchestrating `Ship` (worktree deliverables) and `Scout` (investigation reports) task shapes.
  - [x] Implemented merge authority validation (`LocalOnly`, `DirectPr`, `NoMistakes`) and `turn_end_guard` backstop.
  - [x] Added `crew_dispatch`, `crew_status`, `crew_merge` tools with full JSON schema and Markdown fallback parsing.
  - [x] Added automated unit tests for First Mate task lifecycles, worktree merging, and Herdr OSC state emission.

---

## 2. Completed Architecture & Tool Integration

- [x] **First-Class Neovim Lua Companion (`pi.nvim`)**:
  - [x] Implemented pure Lua plugin interfacing with `pi-rs --rpc` over `vim.fn.jobstart`.
  - [x] Supported visual selection code transformations (`<leader>pa`), floating conversational buffers, and buffer patching (`<leader>pc`, `<leader>pe`, `<leader>pf`, `<leader>pr`).
  - [x] Injected active buffer, LSP diagnostics (`vim.diagnostic.get`), and cursor coordinates into prompt context.
  - [x] Added health check integration (`:checkhealth pi`) and Vim documentation (`doc/pi.txt`).

- [x] **Session Trajectory Replay & Time-Travel**:
  - [x] Exported, simulated, and replayed entire session trajectories with deterministic turn recreation (`--replay`, `pi/session/trajectory`, `pi/session/diff`, `pi/session/simulate_rewind`).

- [x] **Visual Diff Inspector in TUI**:
  - [x] Interactive terminal diff viewer overlay in `pi-tui` (`DiffView`, `DiffViewState`, `/diff` slash command, scroll & accept/reject keybindings).


