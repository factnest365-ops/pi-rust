# ROADMAP.md — Search Upgrade Plan

This roadmap covers the current baseline and the ordered work to move `pi-rust` from a linear turn loop to a verifiable tree-search harness. It replaces the prior marketing-style phase plan because the goal here is execution order, not narrative.

---

## Current Baseline

| Layer | Status | Notes |
| --- | --- | --- |
| Linear turn loop | active | `AgentLoop::run_turn` at `crates/pi-core/src/lib.rs:399` |
| Tool registry | active | `ToolExecutor::tool_definitions()` at `crates/pi-tools/src/lib.rs:105` |
| Tool dispatch | active | `ToolExecutor::execute()` at `crates/pi-tools/src/lib.rs:59` |
| Session DAG | active | `crates/pi-session/src/lib.rs` with JSONL persistence |
| Speculative races | present but not part of main loop | `crates/pi-tools/src/lib.rs:79` and `pi-core` speculative modules |

The main missing capability is search over tool trajectories. Everything below is ordered to add that capability in installable tiers.

---

## Tier 1 — Best-of-N

Goal: improve robustness with minimal structural change.

- `crates/pi-core/src/lib.rs` — wrap candidate generation around `ProviderClient::stream_messages_with_tools`; add `best_of_n` config path through `AgentLoop`/`ModelConfig`
- `crates/pi-providers/src/lib.rs` — add `best_of_n` selection helper near `stream_messages_with_tools` at `crates/pi-providers/src/lib.rs:1058`
- Tests: unit tests for candidate selection and a `cargo test --workspace` gate

Deliverable: `best_of_n` is configurable, defaults to a small bounded N, and selection does not break existing turn behavior.

---

## Tier 2 — MCTS over Tool Prefixes

Goal: replace the linear turn loop with minimal Monte Carlo Tree Search.

- `crates/pi-core/src/lib.rs` — add MCTS state and node types near `AgentLoop`; insert select/expand/simulate/backprop at `crates/pi-core/src/lib.rs:399`
- `crates/pi-tools/src/lib.rs` — keep execution surface stable; use `ToolExecutor::execute()` at `crates/pi-tools/src/lib.rs:59` as the simulation step
- `crates/pi-session/src/lib.rs` — preserve DAG causality and metadata during MCTS rollouts
- Tests: unit tests for UCT/selection/backprop, integration test for a rollout ending in a `bash` verification step, optional feature flag

Deliverable: tree search over tool prefixes with verification as reward and green workspace tests.

---

## Tier 3 — Parallel Herdr + Value Model

Goal: scale evaluation without breaking the core loop.

- `crates/pi-core/src/lib.rs` — value-model interface and rollout scheduling
- `crates/pi-daemon/src/lib.rs` — ambient rollout workers if needed
- `crates/pi-rpc/src/lib.rs` — expose search/rollout status without blocking main paths
- Feature-gate Tier 3 until Tier 2 is stable

Deliverable: parallel rollout evaluation behind a flag, with bounded complexity and preserved leaf-crate decoupling.
