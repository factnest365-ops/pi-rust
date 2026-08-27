# Contributing to tau

Thank you for wanting to contribute. This document will help you get started.

## Quick start

```bash
git clone https://github.com/factnest365-ops/pi-rust.git
cd pi-rust
cargo test --workspace          # run all tests
cargo clippy --all-targets      # check for warnings
cargo build --bin tau           # verify it compiles
```

## Development workflow

1. **Branch off `main`** — never push directly to `main`.
   ```bash
   git checkout -b fm/your-feature-name main
   ```

2. **Make surgical changes** — touch only what you need to change. Don't reformat unrelated code.

3. **Run the full gate before submitting:**
   ```bash
   cargo fmt --all -- --check
   cargo clippy --all-targets -- -D warnings
   cargo test --workspace
   cargo doc --workspace --no-deps
   cargo build --bin tau
   ```

4. **Commit messages** — use conventional commits:
   ```
   feat(pi-core): add MCTS node selection
   fix(pi-tools): handle EOF in bash tool
   docs: update SPEC for daemon IPC
   chore: bump rust edition to 2024
   ```

5. **Open a PR** — describe what changed and why. Link any relevant issues.

## Architecture

See [`SPEC.md`](SPEC.md) for the full architecture spec and [`AGENTS.md`](AGENTS.md) for the quality gates.

### Crate layout

```
crates/
  pi-cli       — binary entry points (tau, pi-rs)
  pi-core      — agent turn loop, memory, planning, undo
  pi-providers — LLM client abstractions (33+ providers)
  pi-session   — DAG session tree, JSONL persistence
  pi-tools     — tool registry and execution
  pi-tui       — ratatui terminal UI
  pi-rpc       — JSON-RPC 2.0 daemon
  pi-daemon    — resident background daemon
```

Dependency flow is strict — leaf crates (`pi-providers`, `pi-session`, `pi-tools`) never depend on each other or on orchestrator crates. See [`AGENTS.md`](AGENTS.md) Invariant 1.

## Adding a new tool

1. Add the tool struct and `execute` method to `crates/pi-tools/src/`.
2. Register it in `ToolExecutor::tool_definitions()` (JSON schema).
3. Add a dispatch arm in `ToolExecutor::execute()`.
4. Add fallback parsing in `AgentLoop::extract_fallback_tool_calls()` in `pi-core`.
5. Add tests.

## Code quality

- **No `unwrap()` in production paths.** Use `?` or `map_err` with descriptive messages.
- **No `TODO` / `FIXME` without a tracking issue.** If it's not done, it's not shipped.
- **Clippy warnings are errors.** `-D warnings` is enforced in CI.
- **Tests cover the contract.** Unit tests for logic, integration tests for workflows.

## Reporting issues

Before filing a bug report:
1. Reproduce with the latest `main`.
2. Include `tau --print "your prompt"` output if relevant.
3. Mention your OS, Rust version (`rustc --version`), and provider/model.

## First-time contributors

PRs that:
- Fix a bug
- Add a test
- Improve docs
- Clean up dead code

...are always welcome. Don't overthink the first one.
