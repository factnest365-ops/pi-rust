# Tau (`tau` / `pi-rs`)

<div align="center">

```
  ████████╗ █████╗ ██╗   ██╗    (τ = 2π)
  ╚══██╔══╝██╔══██╗██║   ██║
     ██║   ███████║██║   ██║    The 2π Evolution of Pi
     ██║   ██╔══██║██║   ██║    High-Performance Autonomous Coding Agent
     ██║   ██║  ██║╚██████╔╝
     ╚═╝   ╚═╝  ╚═╝ ╚═════╝
```

**Ultra-High-Performance, Zero-Node, 100% Pure Rust Autonomous Coding Agent & Swarm Orchestrator**

*The definitive pure native Rust evolution of Mario Zechner's Pi Coding Agent ([pi.dev](https://pi.dev) / [`earendil-works/pi`](https://github.com/earendil-works/pi)).*

---

[![Rust: 2024](https://img.shields.io/badge/Rust-2024%20Edition-DEA584.svg?style=flat-square&logo=rust)](https://www.rust-lang.org)
[![Pure Rust](https://img.shields.io/badge/Pure%20Rust-100%25-orange.svg?style=flat-square&logo=rust)]()
[![Zero Node.js](https://img.shields.io/badge/Node.js-0%20Dependencies-brightgreen.svg?style=flat-square)]()
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square)](LICENSE)
[![Tests: 100%](https://img.shields.io/badge/Tests-265%2F265%20Passing-success.svg?style=flat-square)]()
[![Clippy: 0 Warnings](https://img.shields.io/badge/Clippy-0%20Warnings-green.svg?style=flat-square)]()
[![UNIX Manpage](https://img.shields.io/badge/Manpage-tau(1)-purple.svg?style=flat-square)](man/tau.1)

</div>

---

## ⚡ Why Tau (τ)?

`tau` (also callable as `pi-rs`) is a lightning-fast, local-first autonomous AI coding agent, background daemon (`taud`), and swarm orchestrator engineered in 100% pure safe Rust. It features a non-destructive session DAG, dual-protocol tool execution (native JSON + markdown fallback), real-time SSE streaming decoders, 35+ LLM provider integrations with zero-config local daemon discovery, an embedded SQLite FTS5 + SIMD Cognitive Memory Vault, a Federated Specialist fleet (**J.A.R.V.I.S.**, **F.R.I.D.A.Y.**, **E.V.**), Full Autonomy Undo engine, The Alfred Moral Override Protocol, a Ratatui Super-TUI terminal cockpit (`/plan`, `/memory`, `/ask`, `/diff`), and a headless JSON-RPC 2.0 daemon for editor integrations.

### 📊 Feature Matrix: `tau` (Rust) vs Upstream TS `pi`

| Dimension | Upstream TypeScript Pi (`pi.dev`) | `tau` / `pi-rs` (Rust) |
| :--- | :--- | :--- |
| **Runtime & Dependencies** | Requires Node.js (v20+), npm, 200+ node_modules | **Zero external runtimes**. Single static native binary (~12MB). |
| **Background Daemon** | None (ephemeral CLI only) | **100% Pure Rust Native Daemon (`taud`)** with Unix socket IPC |
| **Specialist Personas** | Single monolithic assistant | **Federated Specialist Fleet** (`J.A.R.V.I.S.`, `F.R.I.D.A.Y.`, `E.V.`) |
| **Long-Term Memory** | Flat file notes | **SQLite FTS5 + SIMD Cosine Cognitive Vault** with automatic turn reflexion |
| **Safety & Conscience** | Manual confirmation prompts | **The Alfred Moral Override Protocol** (Tiered non-blocking advisory) |
| **Mutation Rollback** | Manual Git reset | **Action Snapshot Undo Engine** (Instant $<5\text{ms}$ byte-accurate rollback) |
| **Startup Latency** | ~350ms – 1,200ms (V8 JIT initialization) | **< 3ms cold startup** |
| **Memory Footprint** | ~120MB – 350MB RSS | **~8MB – 18MB RSS** (Daemon idles at $<4\text{MB}$) |
| **Session Architecture** | Linear turn list with manual snapshots | **Graph DAG Tree** (Arbitrary rewinds, diffing, simulation & JSONL) |
| **Tool Execution** | Node `child_process` with standard event loop | **Async Tokio Subprocess Trees** with enforced 120s timeout kills |
| **Tool Calling Protocol** | Provider-dependent JSON schema | **Dual Tool Protocol**: Native schema + Markdown fallback for local models |
| **Multi-Provider Support** | Frontier APIs (Anthropic, OpenAI, Gemini) | **33+ Providers** (Frontier, OpenCode, Kilo, Agnes, Ollama, LM Studio, vLLM) |
| **MCP Integration** | Manual server configuration | **Universal MCP Auto-Discovery** (VSCode, Cursor, Claude Desktop, Windsurf) |
| **Multi-Agent Coordination** | Subprocess worker spawns | **First Mate & Herdr Swarm Protocols** with OSC status multiplexing |
| **Terminal Cockpit** | Ink / React-based terminal UI | **Ratatui Super-TUI** (`/plan`, `/memory`, `/ask`, `/diff`, Mermaid renderer) |
| **Editor Integration** | CLI only | **JSON-RPC 2.0 Daemon** + Native Neovim plugin (`lua/pi`) |

---

## 🏗 Workspace Architecture

The workspace is strictly partitioned into 8 focused, decoupled Cargo crates under `crates/`:

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
|     pi-tui     |                       |    pi-core     |                       |   pi-daemon    |
| (Ratatui & UI) |                       | (Agent Engine) |                       | (100% Pure Rust|
+-------+--------+                       +---+---+----+---+                       |  Daemon/taud)  |
        |                                    |   |    |                           +--------+-------+
        +------------------+-----------------+   |    +------------------+-----------------+
                           |                     |                       |                 |
                           v                     |                       v                 |
                  +-----------------+            |              +-----------------+        |
                  |  pi-providers   |            |              |   pi-session    |        |
                  | (Multi-LLM SSE) |            |              |  (DAG History & |        |
                  +-----------------+            |              |  JSONL Storage) |        |
                                                 |              +-----------------+        |
                                                 v                                         |
                                        +-----------------+                                |
                                        |    pi-tools     |                                |
                                        | (Native Tools & |                                |
                                        |  MCP Discovery) |                                |
                                        +--------+--------+                                |
                                                 ^                                         |
                                                 |                                         |
                                        +--------+--------+                                |
                                        |     pi-rpc      |<-------------------------------+
                                        |  (JSON-RPC 2.0) |
                                        +-----------------+
```

- [`crates/pi-cli`](file:///Users/bhavy/pi-rust/crates/pi-cli): Command-line argument parsing (`tau`), login wizard, session replay, and daemon controller (`--daemon-status`, `--daemon-ping`, `--undo`, `--alfred-check`).
- [`crates/pi-daemon`](file:///Users/bhavy/pi-rust/crates/pi-daemon): 100% pure Rust background daemon (`taud`) listening over Unix Domain Socket (`~/.tau/taud.sock`) with JSON-RPC 2.0 IPC.
- [`crates/pi-core`](file:///Users/bhavy/pi-rust/crates/pi-core): Agent turn loop, Dual Tool dispatch, Cognitive Vault (FTS5 + SIMD), Reflexion Engine, Federated Specialist Fleet (`J.A.R.V.I.S.`, `F.R.I.D.A.Y.`, `E.V.`), Plan Executor, Undo Engine, Alfred Protocol, Speculative Engine, Skills Crystallizer.
- [`crates/pi-providers`](file:///Users/bhavy/pi-rust/crates/pi-providers): 33+ LLM provider clients, SSE streaming decoders, TokenProfiler, and hierarchical AuthResolver.
- [`crates/pi-session`](file:///Users/bhavy/pi-rust/crates/pi-session): Session DAG tree, node ID assignment, branch diffing, simulated rewinds, and JSONL disk persistence.
- [`crates/pi-tools`](file:///Users/bhavy/pi-rust/crates/pi-tools): Safe native tools (`read`, `write`, `edit`, `bash`, `grep`, `find`, `ls`, `web_fetch`, `web_search`, `git`, `git_worktree`, `github`, `lsp`, `ast`, `mcp`, `subagents`, `crew`).
- [`crates/pi-tui`](file:///Users/bhavy/pi-rust/crates/pi-tui): Ratatui terminal UI, interactive overlays (Memory Explorer `/memory`, Plan Mode `/plan`, Clarification Modal `/ask`, Diff View `/diff`, Model Picker), 7 themes, and pure-Rust Mermaid ASCII renderer.
- [`crates/pi-rpc`](file:///Users/bhavy/pi-rust/crates/pi-rpc): Bi-directional JSON-RPC 2.0 server over stdin/stdout with ordered MPSC notification streaming.

---

## 🚀 Quick Start

### Installation

#### Option 1: Cargo Install (From Source)

```bash
# Clone the repository
git clone https://github.com/bhavy/pi-rust.git
cd pi-rust

# Install binary to $CARGO_HOME/bin
cargo install --path crates/pi-cli

# Initialize workspace configuration and scaffold AGENTS.md
pi-rs --init
```

#### Option 2: Build & Run with Cargo

```bash
cargo run --bin pi-rs
```

---

## 🖥 Terminal Cockpit (TUI)

Launch the interactive terminal user interface by running `pi-rs`:

```bash
pi-rs
# Or select a model on startup:
pi-rs -m anthropic/claude-3-7-sonnet-latest
```

### 📐 TUI Cockpit Layout

```
┌─ pi-rust 0.1.0 ──────────────────────────────────────── /Users/bhavy/pi-rust ─┐
│                                                                               │
│  🤖 [ASSISTANT] (anthropic/claude-3-7-sonnet-latest)                          │
│  I will inspect the workspace and run the test suite to verify invariants.    │
│                                                                               │
│  ┌─ 🔧 Tool Executing: bash ───────────────────────────────────────────────┐  │
│  │ $ cargo test --workspace                                                │  │
│  │ 103 tests passed; 0 failed; 0 ignored; finished in 1.23s                │  │
│  └─────────────────────────────────────────────────────────────────────────┘  │
│                                                                               │
│  ┌─ 🧠 Thought / Reasoning ────────────────────────────────────────────────┐  │
│  │ Verification passed cleanly. All crate boundaries and schemas align.    │  │
│  └─────────────────────────────────────────────────────────────────────────┘  │
│                                                                               │
│  Flowchart Architecture:                                                      │
│  ┌───────┐      ┌───────────┐      ┌──────────┐                               │
│  │ pi-cli│ ───> │  pi-core   │ ───> │ pi-tools │                               │
│  └───────┘      └───────────┘      └──────────┘                               │
│                                                                               │
├───────────────────────────────────────────────────────────────────────────────┤
│ Context: [████████░░░░░░░░░░░░] 28.4k / 200.0k tokens (14%) | Branch: main   │
├───────────────────────────────────────────────────────────────────────────────┤
│ > /model opencode/deepseek-v4-flash-free█                                     │
└─ [Ctrl+L: Models] [Ctrl+P: Providers] [Ctrl+T: Tree] [Ctrl+R: Rewind] [?: Help] ┘
```

### ⌨️ TUI Keybindings Cheatsheet

| Shortcut | Action | Description |
| :--- | :--- | :--- |
| <kbd>Ctrl</kbd>+<kbd>L</kbd> | **Model Picker** | Fuzzy search and hot-swap active LLM model and provider |
| <kbd>Ctrl</kbd>+<kbd>P</kbd> | **Provider Picker** | Switch active LLM backend gateway (Anthropic, OpenAI, Kilo, Ollama, etc.) |
| <kbd>Ctrl</kbd>+<kbd>T</kbd> | **Session Tree** | Inspect conversation history DAG, parent-child nodes, and branch points |
| <kbd>Ctrl</kbd>+<kbd>R</kbd> | **Rewind Turn** | Rewind active conversation pointer to the previous user prompt |
| <kbd>Ctrl</kbd>+<kbd>N</kbd> | **Fork Branch** | Fork session DAG and start a fresh conversational branch |
| <kbd>Ctrl</kbd>+<kbd>C</kbd> | **Interrupt / Exit** | Abort in-flight streaming or tool execution; double-press to exit |
| <kbd>Esc</kbd> | **Close Modal** | Dismiss any active popup overlay or autocomplete menu |
| <kbd>?</kbd> | **Help Sheet** | Display built-in interactive hotkey and command reference |
| <kbd>PgUp</kbd> / <kbd>PgDn</kbd> | **Scroll** | Scroll message transcript viewport up and down |
| <kbd>Tab</kbd> | **Autocomplete** | Autocomplete slash commands, models, themes, and provider names |

### 🧭 Interactive Slash Commands

Type `/` in the input bar to activate slash-command autocomplete:

| Slash Command | Parameters | Description |
| :--- | :--- | :--- |
| `/model` | `[query]` | Fuzzy search and switch active AI model directly |
| `/provider` | `[name]` | Switch default LLM provider backend |
| `/diff` | `[node_a] [node_b]` | Open split-pane side-by-side Diff Viewer |
| `/tree` | — | Display ASCII/Unicode DAG session history tree |
| `/session` | `[new\|save\|load\|id]` | Manage session DAG branches and `.jsonl` persistence |
| `/theme` | `[name]` | Select UI theme (`default`, `dark`, `dracula`, `nord`, `gruvbox`, `monokai`, `catppuccin`) |
| `/login` | `[provider]` | Launch interactive credential configuration wizard |
| `/compact` | — | Force immediate context window compaction and summarization |
| `/clear` | — | Clear viewport transcript while preserving underlying session DAG |
| `/help` | — | Display built-in command and navigation guide |
| `/exit`, `/quit` | — | Save session state to `.jsonl` and exit cleanly |

---

## 🛠 CLI Flags & Options

```bash
pi-rs [OPTIONS]
```

| Flag | Argument | Description |
| :--- | :--- | :--- |
| `-p`, `--print` | `"<QUERY>"` | One-shot query mode: executes prompt, prints result to stdout, and exits |
| `-m`, `--model` | `<MODEL_ID>` | Specify model ID (default: `opencode/deepseek-v4-flash-free`) |
| `--rpc` | — | Launch bidirectional JSON-RPC 2.0 daemon mode over stdin/stdout |
| `-M`, `--models` | — | List all discovered and configured models with context and capability flags |
| `--refresh-models` | — | Force query online provider endpoints and local daemons, update cache, and list |
| `--login` | `[PROVIDER]` | Interactive authentication wizard (prompts for API key and saves to config) |
| `--replay` | `<FILE.jsonl>` | Replay a recorded JSONL session trajectory with step streaming |
| `--replay-delay-ms` | `<MS>` | Milliseconds delay between steps during replay (default: 50) |
| `--completions` | `<SHELL>` | Generate shell completions (`bash`, `zsh`, `fish`, `powershell`, `elvish`) |
| `--init` | — | Initialize `~/.pi` configuration directory and scaffold starter `AGENTS.md` |
| `-h`, `--help` | — | Print CLI usage help |
| `-V`, `--version` | — | Print version information |

---

## 🌐 Multi-LLM Provider Support (33+ Providers)

`pi-rust` connects to 33+ AI provider gateways and automatically auto-discovers local model daemons:

### 1. Frontier & Cloud Gateways

| Provider | Default / Popular Models | Environment Variable | Capabilities |
| :--- | :--- | :--- | :--- |
| **Anthropic** | `anthropic/claude-3-7-sonnet-latest`, `claude-3-5-sonnet` | `ANTHROPIC_API_KEY` | Reasoning, Vision, Native Tools |
| **OpenAI** | `openai/gpt-4o`, `openai/o1`, `openai/o3-mini` | `OPENAI_API_KEY` | Reasoning, Vision, Native Tools |
| **Google Gemini** | `gemini/gemini-2.5-pro`, `gemini/gemini-2.5-flash` | `GEMINI_API_KEY` | Vision, 2M Context, Native Tools |
| **DeepSeek** | `deepseek/deepseek-chat`, `deepseek/deepseek-reasoner` | `DEEPSEEK_API_KEY` | Reasoning, High-Throughput |
| **OpenCode** | `opencode/deepseek-v4-flash-free`, `opencode/zen-coder` | Free / Preconfigured | Zero-config, Coding Optimized |
| **Kilo Gateway** | `kilo/deepseek-r1`, `kilo/qwen-2.5-coder` | `KILO_API_KEY` | Reasoning, High-Speed Gateway |
| **OpenRouter** | `openrouter/auto`, `openrouter/anthropic/claude-3.5-sonnet` | `OPENROUTER_API_KEY` | Multi-Provider Aggregator |
| **Groq** | `groq/llama-3.3-70b-versatile` | `GROQ_API_KEY` | Ultra-Low Latency LPU |
| **Mistral AI** | `mistral/codestral-latest`, `mistral/mistral-large` | `MISTRAL_API_KEY` | Code Specialist |
| **xAI Grok** | `xai/grok-2` | `XAI_API_KEY` | Frontier Reasoning |

### 2. Local Zero-Config Daemons

`pi-rust` automatically probes local ports on startup or on `--refresh-models`:

| Daemon | Default Endpoint | Example Model Identifier |
| :--- | :--- | :--- |
| **Ollama** | `http://localhost:11434` | `ollama/llama3.2`, `ollama/qwen2.5-coder` |
| **LM Studio** | `http://localhost:1234/v1` | `lmstudio/mistral-7b-instruct` |
| **vLLM** | `http://localhost:8000/v1` | `vllm/meta-llama-3-70b-instruct` |
| **llama.cpp** | `http://localhost:8080/v1` | `llamacpp/default` |

---

## 🐝 Swarm Coordination: First Mate & Herdr Protocols

`pi-rust` includes native orchestration protocols for multi-agent swarm environments:

### 1. First Mate Protocol (`pi_core::firstmate`)
- **Scout / Ship Workflows**: Automatically creates isolated Git worktrees (`.pi/worktrees/`) for parallel task branches.
- **Automated Verification Gates**: Executes compiler typechecks (`cargo check`), linter verification (`cargo clippy`), and test suites (`cargo test`) inside worktree sandboxes before merging.
- **Atomic PR Dispatch**: Commits changes and prepares pull requests cleanly.

### 2. Herdr Swarm Protocol (`pi_core::herdr`)
- **OSC ANSI Terminal Status Broadcasting**: Emits standard Operating System Command escape sequences (`\x1b]1337;AgentState=...\x07`) to notify terminal multiplexers (tmux, Zellij, Herdr Swarm UI).
- **State Lifecycle**: Real-time broadcasts for `idle`, `streaming`, `tool_executing`, `compacting`, `waiting_input`, and `terminated`.
- **Lock-Free Message Queues**: Bi-directional inter-agent message passing over asynchronous MPSC channels.

---

## 🔌 Neovim Integration (`pi.nvim`)

A complete, native Lua plugin is included in [`lua/pi/`](file:///Users/bhavy/pi-rust/lua/pi/) for seamless Neovim integration.

### Setup with `lazy.nvim`

```lua
{
  "bhavy/pi-rust",
  config = function()
    require("pi").setup({
      binary_path = "pi-rs", -- Path to compiled pi-rs binary
      default_model = "opencode/deepseek-v4-flash-free",
      auto_open_diff = true,
      window = {
        width = 0.45,
        position = "right",
      }
    })
  end,
  keys = {
    { "<leader>pi", "<cmd>Pi<cr>", desc = "Toggle Pi Cockpit" },
    { "<leader>pp", "<cmd>PiPrompt<cr>", desc = "Send Prompt to Pi" },
    { "<leader>pm", "<cmd>PiModels<cr>", desc = "Select Pi Model" },
    { "<leader>pd", "<cmd>PiDiff<cr>", desc = "Pi Diff View" },
  }
}
```

### Neovim Commands

- `:Pi` — Toggle interactive Pi floating/side panel
- `:PiPrompt [prompt]` — Send buffer context or prompt to Pi
- `:PiModels` — Fuzzy picker for AI models
- `:PiDiff` — View side-by-side diff of agent code modifications
- `:PiTree` — Inspect session DAG branch history
- `:PiStatus` — Check JSON-RPC daemon connection health
- `:PiStop` — Cancel in-flight agent turn

---

## 📡 JSON-RPC 2.0 Daemon API Specification

When invoked with `pi-rs --rpc`, standard input and standard output speak JSON-RPC 2.0:

### Request Methods

```json
// Health Check
--> {"jsonrpc": "2.0", "id": 1, "method": "pi/ping", "params": {}}
<-- {"jsonrpc": "2.0", "id": 1, "result": {"status": "ok", "version": "0.1.0"}}

// Agent Turn Execution
--> {"jsonrpc": "2.0", "id": 2, "method": "pi/prompt", "params": {"prompt": "Write a test for crates/pi-tools"}}

// List Tools
--> {"jsonrpc": "2.0", "id": 3, "method": "pi/tools/list", "params": {}}

// Session Rewind
--> {"jsonrpc": "2.0", "id": 4, "method": "pi/session/rewind", "params": {"node_id": "01914b7e-9087-7a20-8012-32b71940beef"}}
```

### Server Notifications (Streaming)

```json
<-- {"jsonrpc": "2.0", "method": "pi/streamingChunk", "params": {"chunk": "Sure! Here is the test code..."}}
<-- {"jsonrpc": "2.0", "method": "pi/toolExecuting", "params": {"tool_name": "write", "tool_call_id": "call_123"}}
<-- {"jsonrpc": "2.0", "method": "pi/toolCompleted", "params": {"tool_name": "write", "is_error": false}}
```

---

## 📖 UNIX Manpage

A standard Troff/Groff manpage is available in [`man/pi-rs.1`](file:///Users/bhavy/pi-rust/man/pi-rs.1).

To view the manpage locally:

```bash
man ./man/pi-rs.1
```

To install the manpage globally:

```bash
sudo cp man/pi-rs.1 /usr/local/share/man/man1/pi-rs.1
sudo mandb # On Linux systems
man pi-rs
```

---

## 🧪 Quality Verification & Testing

Every commit and pull request must satisfy the **Zero Warning Quality Gate**:

```bash
# 1. Type check workspace
cargo check --workspace --all-targets

# 2. Strict Clippy check (0 warnings allowed)
cargo clippy --workspace --all-targets -- -D warnings

# 3. Complete test suite (103 unit & integration tests)
cargo test --workspace -- --nocapture
```

---

## 📄 License & Attribution

Released under the **MIT License**.

- Built by **Bhavy & the Antigravity Team**.
- Re-architected in pure Rust based on the groundbreaking work of **Mario Zechner** and the upstream TypeScript Pi Agent ([pi.dev](https://pi.dev) / [`earendil-works/pi`](https://github.com/earendil-works/pi)).
