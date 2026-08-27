<div align="center">

```
  ████████╗ █████╗ ██╗   ██╗    (τ = 2π)
  ╚══██╔══╝██╔══██║██║   ██║
     ██║   ███████║██║   ██║    The 2π Evolution of Pi
     ██║   ██╔══██║██║   ██║    High-Performance Autonomous Coding Agent
     ██║   ██║  ██║╚██████╔╝
     ╚═╝   ╚═╝  ╚═╝ ╚═════╝
```

[![Rust: 2024](https://img.shields.io/badge/Rust-2024%20Edition-DEA584.svg?style=flat-square&logo=rust)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square)](LICENSE)
[![CI branch](https://img.shields.io/badge/CI-check%20main-green.svg?style=flat-square)](https://github.com/factnest365-ops/pi-rust/actions)
[![PRs](https://img.shields.io/badge/PRs-6%20open-informational.svg?style=flat-square)](https://github.com/factnest365-ops/pi-rust/pulls)
[![Stars](https://img.shields.io/badge/Stars-welcome-yellow.svg?style=flat-square)](https://github.com/factnest365-ops/pi-rust)

A pure Rust autonomous coding agent — zero external runtimes, <10MB runtime, 33+ LLM providers, crew dispatch, and a resident daemon.

[Features](#-why-tau) • [Install](#-install) • [Demo](#-demo) • [Active PRs](#-active-prs) • [Architecture](#-architecture) • [Docs](#-docs) • [Contributing](#-contributing)

</div>

---

## Why Tau (`tau`)

`tau` / `pi-rs` is a lightning-fast, local-first autonomous AI coding agent and background daemon (`taud`), engineered in 100% pure safe Rust. Shipped features:

- Non-destructive session DAG with JSONL persistence
- Dual-protocol tool execution (structured + fallback parsing)
- Real-time SSE provider streaming across 33+ LLMs
- SQLite FTS5 + SIMD cognitive memory vault with reflexion
- Federated specialist personas (`J.A.R.V.I.S.`, `F.R.I.D.A.Y.`, `E.V.`)
- Undo engine with action snapshots
- Alfred moral override protocol
- Ratatui TUI cockpit with overlays
- JSON-RPC 2.0 daemon for editor integrations
- Crew dispatch over isolated git worktrees
- Speculative execution with ghost worktree races
- Resident cron daemon with macOS notifications

Planned (in progress on feature branches):
- MCTS-based tool trajectory search
- Best-of-N completion selection
- Full autonomous goal execution loop

### Feature matrix

| Dimension | Hermes / upstream TS approach | `tau` / `pi-rs` (Rust) |
| :--- | :--- | :--- |
| **Runtime** | Node.js runtime, npm, Electron/Ink layers | **Zero external runtimes**; single native Rust binary |
| **Memory target** | Electron/Node heap, often 120MB+ | **< 10MB** runtime target; daemon idles < 4MB |
| **Terminal UI** | Ink/React terminal UI | **Ratatui TUI** with overlays, themes, and Mermaid rendering |
| **Crew / workers** | Python `ThreadPool` or ad-hoc subprocesses | **First Mate & Herdr crew dispatch** over isolated git worktrees |
| **Long-term memory** | Flat notes, manual recall | **SQLite FTS5 + SIMD Cosine vault** with reflexion and belief revision |
| **Scheduling** | Gateway/cron outside the runtime | **Resident cron daemon** with JSON-RPC hooks and macOS notifications |
| **Safety** | Manual confirmation prompts | **Alfred protocol** + action-snapshot undo + subprocess timeouts |
| **MCP support** | Manual MCP wiring in many setups | **Universal MCP auto-discovery** with stdio/HTTP execution |
| **Editor integration** | CLI-only in many workflows | **JSON-RPC 2.0 daemon** over Unix socket |
| **Build / QA** | TS/JS lint + tests | **Cargo, Clippy, tests**, CI-gated zero-warning policy |

---

## Install

### One-liner

```bash
curl -fsSL https://raw.githubusercontent.com/factnest365-ops/pi-rust/main/install.sh | bash
```

### From source

```bash
git clone https://github.com/factnest365-ops/pi-rust.git
cd pi-rust

cargo install --path crates/pi-cli
pi-rs --init
```

### Run

```bash
pi-rs
```

The installer places `tau` and `pi-rs` in `~/.tau/bin` and keeps existing legacy aliases in `~/.pi/bin` when present.

---

## Demo

> Add `assets/banner.png` and a demo GIF/Screencast at the repo root, then replace this placeholder with an embedded preview. The current README intentionally ships a placeholder only; visual assets can be added without changing behavior.

---

## Active PRs

- `fm/pi-rust-dream3` — MCTS search layer + Best-of-N (Phase 1-2, not yet merged)
- `fm/pi-rust-crew-dispatch` — crew dispatch tools, swarm coordination, and worktree isolation
- `fm/pi-rust-crew-verify` — verification gates, test orchestration, and pre-merge checks
- `fm/pi-rust-daemon-cron` — daemon lifecycle, Unix socket hygiene, and ambient awareness
- `fm/pi-rust-crew-memskill` — cognitive memory vault, reflexion, and skill crystallization

---

## Architecture

```mermaid
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

For full protocol details, see [`SPEC.md`](SPEC.md).

---

## Terminal cockpit

```bash
pi-rs
# Or choose a model on startup:
pi-rs -m anthropic/claude-3-7-sonnet-latest
```

| Shortcut | Action |
| :--- | :--- |
| <kbd>Ctrl</kbd>+<kbd>L</kbd> | Model picker |
| <kbd>Ctrl</kbd>+<kbd>P</kbd> | Provider picker |
| <kbd>Ctrl</kbd>+<kbd>T</kbd> | Session DAG tree |
| <kbd>Ctrl</kbd>+<kbd>R</kbd> | Rewind turn |
| <kbd>Ctrl</kbd>+<kbd>N</kbd> | Fork branch |
| <kbd>Esc</kbd> | Close modal |
| <kbd>?</kbd> | Help sheet |

---

## Multi-LLM providers

`pi-rust` supports 33+ providers and auto-discovers local daemons.

| Provider | Example models | Env variable |
| :--- | :--- | :--- |
| **Anthropic** | `claude-3-7-sonnet-latest` | `ANTHROPIC_API_KEY` |
| **OpenAI** | `gpt-4o`, `o1`, `o3-mini` | `OPENAI_API_KEY` |
| **Gemini** | `gemini-2.5-pro`, `gemini-2.5-flash` | `GEMINI_API_KEY` |
| **DeepSeek** | `deepseek-chat`, `deepseek-reasoner` | `DEEPSEEK_API_KEY` |
| **OpenRouter** | `openrouter/auto` | `OPENROUTER_API_KEY` |
| **Groq** | `llama-3.3-70b-versatile` | `GROQ_API_KEY` |
| **Kilo** | `kilo/deepseek-r1` | `KILO_API_KEY` |
| **Ollama** | `ollama/llama3.2` | local daemon |
| **LM Studio** | `lmstudio/mistral-7b-instruct` | `http://localhost:1234/v1` |
| **vLLM / llama.cpp** | local OpenAI-compatible endpoints | local daemon |

---

## Docs

- [`install.sh`](install.sh) — shell installer used by the one-liner
- [`man/pi-rs.1`](man/pi-rs.1) — Unix manpage
- [`SPEC.md`](SPEC.md) — architecture, protocol, and crate contracts
- [`ROADMAP.md`](ROADMAP.md) — phase plan, milestones, and invariants
- [`AGENTS.md`](AGENTS.md) — contributor agent protocol and quality gates

---

## Contributing

1. Create an isolated branch from `main`.
2. Make minimal, surgical changes.
3. Run `cargo check --workspace --all-targets`.
4. Run `cargo test --workspace -- --nocapture`.
5. Run `cargo clippy --workspace --all-targets -- -D warnings`.
6. Open a PR against `main`; never push to `main` directly.

This repo follows a strict zero-warning policy and expects README, docs, and examples to stay in sync with shipped behavior.
