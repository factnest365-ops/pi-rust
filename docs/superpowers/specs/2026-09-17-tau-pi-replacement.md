# Tau to Pi Replacement Specification

> Status: approved by Boss on 2026-09-17. This spec is the binding architecture for the first implementation phase.

## Goal

Make Tau a credible practical replacement for Pi by fixing the agent operating contract and reliable workflows first, while preserving Tau's Rust and local-first advantages as differentiators.

## Product decision

Tau is a derivative Rust reimplementation/port, not a direct source clone. Do not chase every Tau experiment or reproduce Pi's entire surface before the core is dependable. The first release target is a boring, trustworthy coding agent with explicit optional extensions and a clear experimental boundary.

## Architecture

1. **Deterministic runtime policy.** The normal CLI, TUI, and RPC paths use one policy implementation. Ordinary workspace reads, writes, edits, and shell commands do not require an interactive approval sink. Protected paths and destructive command patterns remain denied. An approval sink is optional and explicit.
2. **Canonical agent contract.** The prompt exposes only active tools with concise snippets. Message roles, tool-call IDs, tool results, project context, compaction summaries, and branch summaries have stable formats. Personas, planning, speculation, and experimental hooks are not injected into every session.
3. **Core tool set.** The minimum supported set is `read`, `write`, `edit`, `bash`, `grep`, `find`, and `ls`. Native tool calling and markdown fallback parsing share semantics. Web, Git, GitHub, LSP, AST, MCP, subagents, and crews are optional integrations.
4. **Session and workflow parity.** Session persistence, branching, fork, rewind, import, resume, naming, scoped models, trust, logout, and package workflows must be real and tested. Pi JSONL compatibility is either migrated or rejected with an explicit versioned message.
5. **Bounded extensions.** The current QuickJS bridge is an optional experimental tool bridge, not full Pi extension compatibility. It remains disabled by default, isolated per call, capability-gated, timeout-limited, and clearly documented.
6. **Data-driven provider and UI metadata.** Provider taxonomy, auth aliases, model capabilities, thinking levels, picker categories, help counts, and theme values come from shared registries/configuration rather than duplicated hardcoded ladders. Model refresh is nonblocking and retains a usable stale cache.
7. **Experimental boundary.** Alfred, federation, speculative worktrees, crews, vault/reflexion, daemon behavior, broad MCP discovery, and similar features are opt-in and labeled experimental until independently verified.

## First-phase acceptance criteria

### Runtime and permissions

- CLI, TUI, and RPC execute an ordinary workspace `bash` command without an `ApprovalSink`.
- Destructive commands and protected paths remain denied by deterministic policy.
- No normal path returns `Permission denied: approval required but no sink is configured`.
- Timeouts and cancellation clean up subprocess descendants.
- RPC stdout contains only valid JSON-RPC frames.
- Provider stream fragmentation, tool errors, and final-response ordering are covered by tests.

### Core agent and tools

- Native and markdown fallback calls produce equivalent results for the seven core tools.
- Session persistence, restart, branch selection, fork, and rewind preserve user/assistant/tool causality.
- Compaction preserves active intent, recent tool results, branch summaries, and file decisions.
- Unknown tools fail clearly.
- The prompt lists only tools available to the model.

### Pi workflows

- `settings`, `scoped-models`, `import`, `clone`, `copy`, `name`, `changelog`, `trust`, `logout`, and `resume` are implemented or explicitly reported unsupported.
- Package `install`, `remove`/`uninstall`, `update`, `list`, and `config` have tested behavior rather than empty stubs.
- A fresh install has a tested `fd`/`ripgrep` provisioning path or a documented built-in equivalent.
- Session import/resume has a versioned compatibility decision.

### Extensions

- Default builds do not load extensions.
- `--no-extensions` disables discovery; explicit extension paths follow a documented contract.
- Extension execution is isolated, capability-gated, timeout-limited, and returns structured errors.
- The bridge is not advertised as full Pi compatibility.

### Provider/model/TUI

- Adding a provider/model in configuration updates CLI listings, picker, inspector, and auth without Rust source changes.
- Unknown providers fail closed.
- Scoped models affect selection and execution.
- Thinking/reasoning effort is propagated to provider requests.
- Model refresh is nonblocking and stale-cache tolerant.
- TUI counts and colors are derived from shared data/palette; hardcoded UI inventory is removed from active paths.

### Experimental features

- Default startup and a basic coding task do not start the daemon, vault, crews, speculation, federation, or reflexion.
- Experimental features require an explicit flag or Cargo feature.
- Each experimental feature has independent tests and documentation labels it alpha/experimental.

## Verification gates

```bash
cargo check --workspace --all-targets
cargo test --workspace --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
cargo build --release --bin tau --bin pi-rs
```

Additional gates include permission/runtime integration tests, session import/resume tests, package-manager tests, extension fixture tests with and without the feature, live-provider smoke tests using a configured non-secret model, RPC stdout validation, and TUI/data-driven inventory checks.

## Non-goals for this phase

- Do not rewrite Tau or change its Rust/local-first architecture.
- Do not implement every Pi command or every Tau experiment before the core is reliable.
- Do not add interactive permission popups.
- Do not merge or push experimental features into the default path.
- Do not treat provider routing or credential failures as Tau defects.
