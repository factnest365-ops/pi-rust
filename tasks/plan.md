# Phase 2 Technical Implementation Plan

## Task Breakdown & Order

### Task 1: Real-time SSE Streaming & Local Model Autodiscovery in `pi-providers`
- Implement SSE response stream handler for Anthropic Messages and OpenAI/compatible Chat Completions.
- Provide `ProviderClient::stream_with_tools` with real-time chunk callbacks and structured tool call assembly.
- Extend `ModelCatalog::get_models` to probe Ollama (`:11434`), llama.cpp (`:8080`), and LM Studio (`:1234`).
- Update `ModelConfig::resolve` to support `llamacpp` and `lmstudio` provider prefixes.
- Unit tests for SSE stream decoding and provider resolution.

### Task 2: Dual Tool Protocol & Streaming Integration in `pi-core`
- Update `AgentLoop::run_turn` to use `ProviderClient::stream_with_tools` and emit `TurnEvent::ModelStreaming`.
- Implement robust multi-format markdown fallback tool parser (for `bash`, `write`, `edit`, `read`) when structured tool calls are absent.
- Ensure tool execution outputs are recorded into `SessionTree` and passed recursively back into model context.
- Unit tests for fallback extraction and multi-tool markdown execution.

### Task 3: Interactive Streaming & Interruption Handling in `pi-tui`
- Wire real-time streaming chunks into live TUI transcript updates.
- Implement async task execution with cancellation handle: pressing `Escape` interrupts active generation immediately and returns control to user without freezing or crashing.
- Support autodiscovered models in the interactive Model Picker dialog (`Ctrl+L` / `Ctrl+R`).

### Task 4: Comprehensive Workspace Verification & Code Review ✅
- Run `cargo test --workspace` across all crates.
- Run `cargo check --workspace --all-targets` and `cargo clippy --workspace --all-targets`.
- Conduct subagent code review across correctness, readability, architecture, security, and performance.
- Update `ROADMAP.md` marking Phase 2 as completed.

---

# Phase 3 Technical Implementation Plan (Native Pi Packages)

### Task 3.1: `pi-git` — Pure Rust Git Integration
- Implement pure Rust Git operations using `gix` / Git CLI.
- Add tools for staging (`git_stage`), hunk inspection (`git_diff`), and AI commit synthesis (`git_commit`).
- Unit and integration tests in `crates/pi-tools`.

### Task 3.2: `pi-web` — Headless Web Scraping & Article Extraction
- Build fast headless HTTP article extractor using connection-pooled `reqwest` and HTML-to-markdown parser.
- Add `web_search` and `web_fetch` tools to `pi-tools`.
- Verify zero headless browser overhead.

### Task 3.3: `pi-tokens` — BPE Tokenization Engine
- Integrate `tiktoken-rs` for exact BPE token counting (cl100k_base, o200k_base, and Anthropic estimators).
- Provide accurate context window usage and compaction thresholds in `pi-providers`.

### Task 3.4: `pi-lsp` — Language Server Protocol Bridge
- Implement native LSP client over stdio for Rust (`rust-analyzer`), TypeScript (`tsserver`/`vtsls`), Python (`pyright`/`basedpyright`).
- Provide tools for `lsp_diagnostics`, `lsp_definitions`, `lsp_references`.

### Task 3.5: `pi-github` — PR & Issue Automation
- Bridge to GitHub REST API / `gh` CLI for PR reviews, issue triage, and workflow run checks.
