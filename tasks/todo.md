# Phase 2 Tasks ✅ *(Completed)*

- [x] Task 1: SSE Streaming & Local Model Autodiscovery in `pi-providers`
  - Acceptance: `ProviderClient::stream_with_tools` streams SSE chunks, parses tool calls for Anthropic + OpenAI protocols, and `ModelCatalog` autoprobes Ollama/llama.cpp/LM Studio.
  - Verify: `cargo test -p pi-providers`
  - Files: `crates/pi-providers/src/lib.rs`

- [x] Task 2: Dual Tool Protocol & Streaming Turn Execution in `pi-core`
  - Acceptance: `AgentLoop::run_turn` emits `TurnEvent::ModelStreaming` events and supports markdown fallback extraction for `bash`, `write`, `edit`, `read`, `grep`, `find`, `ls`.
  - Verify: `cargo test -p pi-core`
  - Files: `crates/pi-core/src/lib.rs`

- [x] Task 3: Interactive Streaming & Interruption in `pi-tui`
  - Acceptance: Live streaming token updates in transcript, `Escape` interrupts in-flight turn cleanly, Model Picker displays discovered local/gateway models.
  - Verify: `cargo check -p pi-tui && cargo test -p pi-tui`
  - Files: `crates/pi-tui/src/lib.rs`

- [x] Task 4: Multi-Axis Verification, Subagent Code Review & Roadmap Update
  - Acceptance: `cargo check --workspace --all-targets` (0 warnings), `cargo test --workspace` (all pass), `cargo clippy --workspace --all-targets` (0 lints), and `ROADMAP.md` updated.
  - Verify: `cargo test --workspace && cargo clippy --workspace --all-targets`
  - Files: `ROADMAP.md`

---

# Phase 3 Tasks ✅ *(Completed)*

- [x] Task 3.1: `pi-git` — Pure Rust Git Integration
  - Acceptance: Smart git staging, hunk diffing, and automated commit message synthesis via pure Rust Git module.
  - Verify: `cargo test -p pi-tools`
  - Files: `crates/pi-tools/src/git.rs`

- [x] Task 3.2: `pi-web` — Headless Web Fetch & Article Parser
  - Acceptance: Zero-browser web scraping, HTTP article extraction, and HTML-to-markdown conversion using connection-pooled `reqwest`.
  - Verify: `cargo test -p pi-tools`
  - Files: `crates/pi-tools/src/web.rs`

- [x] Task 3.3: `pi-tokens` — BPE Context Profiler
  - Acceptance: Native BPE token counting for OpenAI/Anthropic/DeepSeek context estimators & budget computation.
  - Verify: `cargo test -p pi-providers`
  - Files: `crates/pi-providers/src/tokens.rs`

- [x] Task 3.4: `pi-skills` — Skill Autodiscovery & Indexing Engine
  - Acceptance: Pure Rust autodiscovery and frontmatter parsing for `~/.pi/agent/skills/` and local `.pi/skills/`.
  - Verify: `cargo test -p pi-core`
  - Files: `crates/pi-core/src/skills.rs`

- [x] Task 3.5: `pi-github` — PR & Issue Automation
  - Acceptance: GitHub API / `gh` CLI bridge for reviewing pull requests, inspecting workflows, and creating issues.
  - Verify: `cargo test -p pi-tools`
  - Files: `crates/pi-tools/src/github.rs`

---

# Phase 4 Tasks ✅ *(Completed)*

- [x] Task 4.1: Syntax Highlighting for Markdown & Code Blocks in `pi-tui`
  - Acceptance: Token-aware syntax colorizer and code block formatter for Rust, JS/TS, Python, JSON in Ratatui transcript.
  - Verify: `cargo test -p pi-tui`
  - Files: `crates/pi-tui/src/lib.rs`

- [x] Task 4.2: Full JSON-RPC 2.0 Engine & Dispatcher in `pi-rpc`
  - Acceptance: Bi-directional JSON-RPC 2.0 methods (`pi/prompt`, `pi/models`, `pi/tools/*`, `pi/skills/*`, `pi/session/*`) with real-time streaming notifications.
  - Verify: `cargo test -p pi-rpc`
  - Files: `crates/pi-rpc/src/lib.rs`

- [x] Task 4.3: AST Slicing & Symbol Navigation in `pi-tools`
  - Acceptance: Exact identifier matching, brace & indentation scope tracking, and structural file outlining in `AstTool` and `LspTool`.
  - Verify: `cargo test -p pi-tools`
  - Files: `crates/pi-tools/src/ast.rs`, `crates/pi-tools/src/lsp.rs`

