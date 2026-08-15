# TASKS.md — Project Task Tracker & Execution Board

## Current Status Overview
- **Project**: `pi-rust` (`pi-rs`) — 100% Pure Rust Port of Mario Zechner's Pi Coding Agent ([pi.dev](https://pi.dev) / [`earendil-works/pi`](https://github.com/earendil-works/pi))
- **Target Parity**: Complete functional, aesthetic, protocol, and architectural parity with `@earendil-works/pi`.
- **Compiler/Lint Status**: Zero warnings, 100% Clippy clean (`cargo clippy --workspace --all-targets -- -D warnings`), 100% test pass rate (64/64 tests passing) across all workspace crates.

---

## 1. Recently Completed Tasks (Sprint Hardening & Protocol Parity) ✅

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

## 2. Active Engineering Tasks

- [x] **First-Class Neovim Lua Companion (`pi.nvim`)**:
  - [x] Implemented pure Lua plugin interfacing with `pi-rs --rpc` over `vim.fn.jobstart`.
  - [x] Supported visual selection code transformations (`<leader>pa`), floating conversational buffers, and buffer patching (`<leader>pc`, `<leader>pe`, `<leader>pf`, `<leader>pr`).
  - [x] Injected active buffer, LSP diagnostics (`vim.diagnostic.get`), and cursor coordinates into prompt context.
  - [x] Added health check integration (`:checkhealth pi`) and Vim documentation (`doc/pi.txt`).

- [x] **Session Trajectory Replay & Time-Travel**:
  - [x] Exported, simulated, and replayed entire session trajectories with deterministic turn recreation (`--replay`, `pi/session/trajectory`, `pi/session/diff`, `pi/session/simulate_rewind`).

- [x] **Visual Diff Inspector in TUI**:
  - [x] Interactive terminal diff viewer overlay in `pi-tui` (`DiffView`, `DiffViewState`, `/diff` slash command, scroll & accept/reject keybindings).

