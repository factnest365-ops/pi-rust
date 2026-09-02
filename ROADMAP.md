# ROADMAP.md — Master Architecture & Self-Improving Meta Super-Harness

> **Design Philosophy**: *"100% Pure Rust — Zero Runtime Bloat, Zero Node.js, Maximum Velocity, Continuous Self-Evolution."*  
> **Tau (`τ`)** is engineered as the definitive, zero-overhead foundation for local-first, self-improving agentic coding.

---

## Strategic Architecture Blueprint

```
                                  ┌─────────────────────────────────────────────────────────────┐
                                  │            Tau (τ) Meta Super-Harness Engine                │
                                  │         100% Pure Rust · Zero Node.js · < 10MB RAM          │
                                  └──────────────────────────────┬──────────────────────────────┘
                                                                 │
         ┌───────────────────────────────────────┬───────────────┴───────────────┬───────────────────────────────────────┐
         ▼                                       ▼                               ▼                                       ▼
┌─────────────────────────────────┐   ┌─────────────────────────────────┐   ┌─────────────────────────────────┐   ┌─────────────────────────────────┐
│   Phase 10: Cognitive Memory    │   │   Phase 11: Plan & Spec Mode    │   │   Phase 12: Super-TUI Cockpit   │   │   Phase 13: Speculative Engine  │
│  - Hybrid FTS5 + SIMD Cosine    │   │  - Stateful task checklist      │   │  - Live Plan & Todo Overlay     │   │  - Parallel Git ghost worktrees │
│  - Continuous turn reflexion    │   │  - Compiler verification gate   │   │  - Cognitive Memory Explorer    │   │  - Automated compiler race      │
│  - Automatic belief revision    │   │  - Compaction-proof state       │   │  - Clarification Modal (/ask)   │   │  - Side-by-side diff arbitrator │
│  - Anti-pattern counter-rules   │   │  - AST Dependency Topology      │   │  - Surgical AST Diff Reviewer   │   │  - Zero-latency auto-merge      │
└─────────────────────────────────┘   └─────────────────────────────────┘   └─────────────────────────────────┘   └─────────────────────────────────┘
```

---

## 1. Completed Core Parity Milestones (Phases 1–9)

### Phase 1: Core Architecture & Reliability ✅
- [x] Strict workspace crate isolation (`pi-cli`, `pi-core`, `pi-providers`, `pi-session`, `pi-tools`, `pi-tui`, `pi-rpc`).
- [x] Stream-buffered, 1-indexed file operations with boundary safety (`pi-tools`).
- [x] Non-linear session tree graph with active branch traversal, fork points, and rewinding (`pi-session`).
- [x] Multi-step agentic turn loop with recursive tool result feedback (`pi-core`).
- [x] HTTP connection pooling (`OnceLock<reqwest::Client>`) and HTTP status error handling (`pi-providers`).
- [x] 100% clean builds with zero compiler warnings and zero Clippy lints across 198+ tests.

### Phase 2: Native Tool Protocol & Real-Time Streaming ✅
- [x] **Real-time Server-Sent Events (SSE) Streaming**: Low-latency token-by-token emission for Anthropic, OpenAI, Gemini, DeepSeek, OpenRouter, Kilo, and Ollama.
- [x] **Dual Tool Protocol**: Native schema-based tool calling (`tools` array) with automatic fallback to markdown code-block execution for all core tools (`bash`, `write`, `edit`, `read`, `grep`, `find`, `ls`).
- [x] **Interruptible Async Execution**: Non-blocking Tokio task orchestration with clean `Escape` / steering input interruption.
- [x] **Local Model Autodiscovery**: Live probing of Ollama (`localhost:11434`), llama.cpp (`localhost:8080`), and LM Studio (`localhost:1234`) endpoints.

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

### Phase 9: Neovim Companion Integration (`pi.nvim`) & Trajectory Replay ✅
- [x] **First-Class Neovim Lua Plugin (`pi.nvim`)**: Pure Lua Neovim companion communicating asynchronously over `--rpc` (visual range prompts, floating chat, buffer patching, LSP diagnostics context, `:checkhealth pi`).
- [x] **Deterministic Session Trajectory Replay**: Export, simulate, and replay entire session trajectories with deterministic turn recreation (`--replay`, `pi/session/trajectory`, `pi/session/diff`).
- [x] **Native Trajectory Diff Visualizer**: In-terminal visual diff viewer for proposed tool modifications and workspace changes (`DiffView`, `/diff` slash command, scroll & keyboard actions).

---

## 2. Meta Super-Harness & Self-Improving Milestones (Phases 10–15)

### Phase 10: Cognitive Memory Vault & Continuous Turn-by-Turn Reflexion ✅
*The memory and learning core that makes Tau continuously smarter with every turn.*
- [x] **Embedded SQLite FTS5 + SIMD Cosine Vault (`~/.tau/vault.sqlite`)**:
  - Zero-daemon, in-process SQLite storage with FTS5 BM25 full-text indexing + compact vector similarity.
  - 3-tier memory scopes: `Global User Profile`, `Repository Invariants`, and `Episodic Session Memories`.
- [x] **Continuous Turn-by-Turn Reflexion Engine**:
  - Automatically captures compiler errors (`cargo check`, `tsc`), runtime panics, and user corrections.
  - Synthesizes root-cause counter-rules (e.g. *"When editing string slicing in pi-tui, always use floor_char_boundary"*) and updates the vault.
- [x] **Belief Revision & Anti-Pattern Deprecation**:
  - When an architectural decision or prompt changes, older superseded rules are automatically marked obsolete with temporal validity windows.
- [x] **Pre-Prompt Hindsight Injection**:
  - Hybrid Reciprocal Rank Fusion (RRF) pre-retrieves top 3–5 high-signal memories and injects them under `[Hindsight Memory & Rules]` before generation starts.
- [x] **Autonomous Skill Crystallization (`/learn`, `/distill`)**:
  - Automatically distills multi-step problem-solving trajectories into reusable local `SKILL.md` templates.

### Phase 11: Plan Mode & Stateful Task Verification Pipeline ✅
*Structured, provable execution of complex goals without drifting.*
- [x] **Stateful Plan & Task Engine (`PlanExecutor`)**:
  - Automatically decomposes high-level user prompts into structured dependency tasks with states: `Pending [ ]`, `Running [◐]`, `Completed [✔]`, `Failed [✖]`.
- [x] **Compaction-Proof Task Persistence**:
  - Active task checklist survives context compaction, session restarts, and `/refresh` without losing state.
- [x] **Automated Verification Gates**:
  - Each task phase executes automated validation commands (`cargo check`, unit tests, lint checks) before transitioning to `Completed`.
- [x] **Rollback on Failure**:
  - If a task step fails verification, the engine initiates a self-repair turn or cleanly rolls back to the last stable DAG checkpoint.

### Phase 12: Super-TUI Cockpit & Interactive Ergonomics ✅
*A developer experience inspired by the best of `omp.sh`, `jcode`, and `pi.dev`.*
- [x] **Live Plan / Todo Overlay Widget (`/plan`)**:
  - Interactive, collapsible task checklist rendered directly above/below the transcript showing live execution progress.
- [x] **Cognitive Memory Explorer (`/memory`)**:
  - Searchable interactive overlay to view, tag, edit, and delete stored facts and repository rules.
- [x] **Structured User Clarification Modal (`/ask`)**:
  - Interactive keyboard-navigable single-choice or multi-choice questionnaire modal in the TUI rather than guessing.
- [x] **Surgical AST Diff Reviewer (`/diff`)**:
  - Dual-pane and side-by-side syntax-highlighted diff viewer with hunk jumping (`n`/`p`) and selective hunk staging.
- [x] **Lazy Skill & Tool Drawer (`/skills`)**:
  - Dynamic JIT skill browser showing active mounted skills and on-demand injection.

### Phase 13: Speculative Execution & Ghost Worktree Racing (`/speculate`) ✅
*Explore alternative architectures concurrently and pick the provably fastest, zero-error solution.*
- [x] **Parallel Ghost Branch Spawner**:
  - Concurrently forks 2–3 competing implementation strategies into ephemeral Git worktrees (`.tau/worktrees/spec-a`, `.tau/worktrees/spec-b`).
- [x] **Automated Compiler & Test Race Arbitrator**:
  - Runs parallel build checks, unit tests, and performance benchmarks across all speculative branches.
  - Automatically selects and merges the winning 100% passing solution with zero regressions.
- [x] **Interactive Split-Diff Review Fallback**:
  - Displays a side-by-side split diff in `pi-tui` showing metrics and lines changed for instant developer selection.

### Phase 14: Code Knowledge Graph (CKG) & Context Slicing Topology ✅
*Structure-aware code intelligence that reduces context token waste by up to 90%.*
- [x] **Pure Rust AST Call & Import Topology**:
  - In-memory CodeGraph indexing function definitions, trait implementations, struct hierarchies, and cross-file references.
- [x] **Topological Dependency Context Slicer**:
  - Extracts only the exact upstream and downstream symbol dependencies needed for the active task, eliminating full-file prompt bloat.
- [x] **Co-Edit Cluster Predictor**:
  - Analyzes Git history to identify files that frequently change together, proactively suggesting related edits.

### Phase 15: 100% Pure Rust Daemon (`taud`) & JARVIS Architecture ✅
*Zero-overhead background ambient intelligence, federated specialists, and moral conscience.*
- [x] **100% Pure Rust Background Daemon (`crates/pi-daemon` / `taud`)**:
  - Persistent Unix Domain Socket IPC (`~/.tau/taud.sock`), JSON-RPC 2.0 loop, $<4\text{MB}$ idle RSS.
- [x] **Federated Specialist Fleet (`J.A.R.V.I.S.`, `F.R.I.D.A.Y.`, `E.V.`)**:
  - Shared cognitive vault with distinct specialized personas and automatic goal routing.
- [x] **Full Autonomy Undo & Rollback Engine (`UndoEngine`)**:
  - Action snapshot journal with instant single-step and multi-step byte-accurate file rollbacks.
- [x] **Cognitive State Sync & Git Fragmentation**:
  - Automated Git versioning for `~/.tau/` (skills, vault, reflexion counter-rules).
- [x] **The Alfred Moral Override Protocol (`AlfredProtocol`)**:
  - Tiered non-blocking conscience evaluation (`Observation` $\to$ `Advisory` $\to$ `Urgent` $\to$ `LastStand`).

---

## 3. Quality & Performance Invariants

- **Memory Invariant**: Base resident set size (RSS) must strictly remain **$< 10\text{ MB}$** and daemon idle footprint **$< 4\text{ MB}$**.
- **Latency Invariant**: Cold-start initialization time must remain **$< 15\text{ ms}$**.
- **Safety Invariant**: 100% Safe Rust across all production code paths.
- **Clippy Policy**: Zero warnings on `cargo clippy --workspace --all-targets -- -D warnings`.
- **Test Invariant**: 100% pass rate across workspace unit and integration test suites (249+ passing tests).
