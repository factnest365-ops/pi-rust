# ROADMAP.md — Engineering Roadmap & Parity Milestones

> **Design Philosophy**: *"100% Pure Rust — Zero Runtime Bloat, Zero Node.js, Maximum Velocity."*  
> `pi-rust` is engineered as the definitive, zero-overhead foundation for local-first, high-performance agentic coding.

---

## Strategic Milestones Overview

```
Phase 1: Workspace Architecture & Reliability ✅
   │
Phase 2: Real-Time SSE Streaming & Dual Tool Protocol ✅
   │
Phase 3: Native Rust Ports of Pi Packages ✅
   │
Phase 4: Code Intelligence & JSON-RPC 2.0 Engine ✅
   │
Phase 5: Universal MCP Client & Multi-Agent Auto-Discovery ✅
   │
Phase 6: Multi-Provider Ecosystem & 33+ Gateways (@earendil-works/pi Parity) ✅
   │
Phase 7: Protocol Hardening, Compaction & Non-Blocking TUI ✅
   │
Phase 8: First Mate & Herdr Swarm Coordination & Worktree Isolation ✅
   │
Phase 9: Neovim Companion Integration (`pi.nvim`) & Trajectory Replay ✅
```

---

## Completed Parity Milestones

### Phase 1: Core Architecture & Reliability ✅
- [x] Strict workspace crate isolation (`pi-cli`, `pi-core`, `pi-providers`, `pi-session`, `pi-tools`, `pi-tui`, `pi-rpc`).
- [x] Stream-buffered, 1-indexed file operations with boundary safety (`pi-tools`).
- [x] Non-linear session tree graph with active branch traversal, fork points, and rewinding (`pi-session`).
- [x] Multi-step agentic turn loop with recursive tool result feedback (`pi-core`).
- [x] HTTP connection pooling (`OnceLock<reqwest::Client>`) and HTTP status error handling (`pi-providers`).
- [x] 100% clean builds with zero compiler warnings and zero Clippy lints.

### Phase 2: Native Tool Protocol & Real-Time Streaming ✅
- [x] **Real-time Server-Sent Events (SSE) Streaming**: Low-latency token-by-token emission for Anthropic, OpenAI, OpenRouter, Kilo, Agnes, and Ollama.
- [x] **Dual Tool Protocol**: Native schema-based tool calling (`tools` array) with automatic fallback to markdown code-block execution for all core tools (`bash`, `write`, `edit`, `read`, `grep`, `find`, `ls`).
- [x] **Interruptible Async Execution**: Non-blocking Tokio task orchestration with clean `Escape` / steering input interruption.
- [x] **Local Model Autodiscovery**: Live probing of Ollama (`localhost:11434`), llama.cpp (`localhost:8080`), and LM Studio (`localhost:1234`) endpoints to dynamically populate the model catalog.

### Phase 3: Pure Rust Native Ports of Pi Packages ✅
- [x] **`pi-git`**: Smart git staging, hunk diffing, and automated conventional commit synthesis via pure Rust Git engine.
- [x] **`pi-github`**: Pull request reviews, issue triage, and workflow run inspections via direct GitHub API / `gh` CLI bridge.
- [x] **`pi-web`**: Fast headless web page fetch and HTML-to-markdown article extractor via connection-pooled `reqwest` (zero headless browser bloat).
- [x] **`pi-tokens`**: Fast BPE token estimation and context budget compaction profiler across model architectures.
- [x] **`pi-skills`**: Pure Rust autodiscovery and frontmatter parsing for `~/.pi/agent/skills/` and local `.pi/skills/`.

### Phase 4: Code Intelligence, AST Slicing & JSON-RPC 2.0 Engine ✅
- [x] **`pi-lsp`**: Native Language Server Protocol bridge for compiler diagnostics (`cargo check`, `py_compile`, `node --check`), document symbols, exact definitions, and hover docs.
- [x] **AST-Aware Slicing (`pi-ast`)**: Syntactic function/class/block slicing with exact identifier boundary matching and brace/indentation scope tracking.
- [x] **Full JSON-RPC 2.0 Engine (`pi-rpc`)**: Bi-directional RPC server (`pi/prompt`, `pi/models`, `pi/tools/*`, `pi/skills/*`, `pi/session/*`) with real-time streaming notifications over standard I/O.
- [x] **Syntax-Highlighted TUI Transcript**: Multi-language syntax token coloring (Rust, TS/JS, Python, JSON, Bash) and code block borders in `pi-tui`.

### Phase 5: Model Context Protocol (MCP) & Multi-Agent Extensibility ✅
- [x] **Native Universal MCP Client**: Full support for Model Context Protocol servers over standard I/O (stdio) and stateless HTTP/SSE.
- [x] **Multi-Agent MCP Auto-Discovery**: Automatic discovery and schema normalization across MCPorter, Gemini/Cloud Code, VS Code, Claude Code, Hermes, LM Studio, and Pi Native configs.
- [x] **Dynamic Tool Ingestion & Execution**: Dynamic schema querying (`tools/list`) and execution dispatch (`tools/call`) with process isolation.
- [x] **TUI & JSON-RPC Management**: Interactive `/mcp` slash command in `pi-tui` and `pi/mcp/list` RPC endpoint.

### Phase 6: Universal Multi-Provider Engine & Dynamic Model Catalog ✅
- [x] **33+ Provider Implementations (@earendil-works/pi-ai v0.84.2 Parity)**: Anthropic, OpenAI, Gemini, Vertex, DeepSeek, Groq, Cerebras, Mistral, xAI, OpenRouter, Bedrock, Copilot, Kilo, Ollama, LM Studio, llama.cpp, vLLM, OpenCode, Agnes, Together, Fireworks, Qwen, Xiaomi MiMo, etc.
- [x] **Dynamic Auto Model Discovery & Refresher (`ModelCatalogLoader`)**: Automated probing of local daemons and remote OpenRouter endpoints with local disk caching (`~/.pi/models_cache.json`).
- [x] **User Custom Models Overrides**: Native parsing of `~/.pi/agent/models.json` and `~/.pi/models.json`.
- [x] **Interactive Searchable Model Picker Overlay (`Ctrl+L`) & Provider Picker (`Ctrl+P`)**: Live fuzzy filtering, provider tags, reasoning/vision capability badges, and context budget indicators.
- [x] **Unified Multi-Tier Auth & Login Flow (`/login`, `/auth`, `--login`)**: Credential hierarchy across env vars, `~/.pi/config.json`, `~/.pi/agent/auth.json`, and `~/.claude.json`.

### Phase 7: Protocol Hardening, Compaction & Non-Blocking TUI ✅
- [x] **OpenAI & Anthropic Native Tool Calling Compliance**:
  - Attached `tool_call_id` to `role: "tool"` messages and structured `tool_calls` to assistant messages to satisfy OpenAI/DeepSeek/Groq protocol specifications.
  - Formatted Anthropic `tool_result` content blocks inside user messages matching Messages API requirements.
- [x] **Automated Context Compaction Loop**:
  - Integrated `TokenProfiler` with automatic turn boundary cut-point discovery (`findTurnStartIndex`).
  - Automated synthetic summary branch creation at 80% context window threshold.
- [x] **Async Subprocess Safety & Zombie Process Prevention**:
  - Converted `execute_bash` to async `tokio::process::Command` with 120s timeout and process killing.
  - Guaranteed MCP child process termination (`child.kill().await`) on timeouts and errors.
- [x] **Ordered JSON-RPC Stream Delivery**:
  - Serialized streaming chunk notifications through an unbounded MPSC queue before final response framing.
- [x] **Non-Blocking TUI Background Operations**:
  - Converted `Ctrl+R` and `/refresh` to non-blocking background Tokio tasks.
- [x] **Tool Defenses**:
  - Disambiguation check for `edit` tool (errors when `occurrences > 1`).
  - Grep `-e` flag protection against hyphen injection.

### Phase 8: First Mate & Herdr Swarm Coordination & Worktree Isolation ✅
- [x] **Herdr Terminal Protocol (`HerdrProtocol`)**:
  - Terminal OSC escape emission (`\x1b]1337;SetUserVar=herdr_state=...\x07\x1b]0;pi [...]\x07`) for agent state awareness (`working`, `blocked`, `done`, `idle`) inside `herdr` multiplexer.
  - Environment detection for `HERDR_SESSION`, `TMUX`, `ZELLIJ`, and `FIRSTMATE_HOME`.
- [x] **First Mate Distro & Crew Dispatch (`FirstMateDistro`)**:
  - Task shape orchestration: `Ship` (code deliverables on dedicated git worktree) vs `Scout` (investigations/reports).
  - Merge authority modes: `LocalOnly` (validation pipeline check before merge), `DirectPr` (`gh pr create`), and `NoMistakes`.
  - Turn-end backstop (`turn_end_guard`) preventing blind stops when crew fleet work is in-flight.
- [x] **Disposable Git Worktree Isolation**:
  - Automatic worktree lifecycle (`git_worktree_create`, `git_worktree_merge`, `git_worktree_remove`) with conflict diagnostics.
- [x] **Dual Tool Suite (`crew_dispatch`, `crew_status`, `crew_merge`)**:
  - Full schema-based tool calling and Markdown fallback parsing in `pi-core` & `pi-tools`.

---

## Active & Upcoming Roadmap

### Phase 9: Neovim Companion Integration (`pi.nvim`) & Trajectory Replay ✅
- [x] **First-Class Neovim Lua Plugin (`pi.nvim`)**: Pure Lua Neovim companion communicating asynchronously over `--rpc` (visual range prompts, floating chat, buffer patching, LSP diagnostics context, `:checkhealth pi`).
- [x] **Deterministic Session Trajectory Replay**: Export, simulate, and replay entire session trajectories with deterministic turn recreation (`--replay`, `pi/session/trajectory`, `pi/session/diff`).
- [x] **Native Trajectory Diff Visualizer**: In-terminal visual diff viewer for proposed tool modifications and workspace changes (`DiffView`, `/diff` slash command, scroll & keyboard actions).

