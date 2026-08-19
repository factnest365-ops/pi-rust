# Implementation Plan: 100% Pure Rust Daemon (`taud`) & JARVIS Architecture

## Overview
Implement a 100% pure Rust background daemon (`crates/pi-daemon` / `taud`) that provides always-on ambient monitoring, Unix domain socket / JSON-RPC IPC, federated specialist sub-agent fleet orchestration (JARVIS, FRIDAY, E.V.), GitHub cognitive state versioning, full undo/rollback engine, and the Alfred moral override protocol.

---

## Architecture & Component Dependency Graph

```
                             +-----------------------------------+
                             |               taud                |
                             |      (100% Pure Rust Daemon)      |
                             +-----------------+-----------------+
                                               |
         +-------------------------------------+-------------------------------------+
         |                                     |                                     |
         v                                     v                                     v
+------------------+                  +------------------+                  +------------------+
|   Daemon Core    |                  | Federated Fleet  |                  |   State Sync &   |
| (Unix Socket IPC |                  | (JARVIS, FRIDAY, |                  |  Rollback Engine |
| & Ambient Select)|                  |  E.V. Specialists|                  |  (Git / Undo)    |
+--------+---------+                  +--------+---------+                  +--------+---------+
         |                                     |                                     |
         +-------------------------------------+-------------------------------------+
                                               |
                                               v
                                      +------------------+
                                      |     pi-core      |
                                      | (Vault, Plan,    |
                                      |  Reflexion, AST) |
                                      +------------------+
```

---

## Phases & Sequential Implementation Order

### Phase 1: Daemon Core & Unix Domain Socket IPC (`crates/pi-daemon`)
- Add new crate `crates/pi-daemon` with binary `taud`.
- Unix domain socket server at `~/.tau/taud.sock` with graceful shutdown (`SIGTERM`, `SIGINT`).
- Bi-directional JSON-RPC 2.0 protocol over Unix socket connecting to `pi-core` engine.
- Ambient event loop with non-blocking 50ms polling, monitoring workspace files and cognitive state.

### Phase 2: Federated Specialist Sub-Agents (`crates/pi-core/src/federation.rs`)
- Implement `SpecialistIdentity`:
  - `J.A.R.V.I.S.` (Engineering, Architecture, Speculative Code Execution, Witty British Persona).
  - `F.R.I.D.A.Y.` (Tactical Analysis, Rapid Security Audit, Zero-Banter Brevity).
  - `E.V.` (Personal Companion, Cognitive State / Fatigue Monitoring, Empathetic Support).
- Central dispatch loop routing goals to the optimal specialist while sharing a unified `TauVault`.

### Phase 3: Full Autonomy & Rollback Engine (`crates/pi-core/src/undo.rs`)
- Implement `UndoEngine` capturing `ActionSnapshot` before any mutation.
- File-level snapshots via ephemeral Git blobs and worktrees.
- Support `undo(action_id)`, `undo_last(n)`, and `preview_undo(action_id)` diff generation.

### Phase 4: GitHub Cognitive State Fragmentation (`crates/pi-core/src/sync.rs`)
- Implement automated Git version control for `~/.tau/` (vault, skills, reflexion counter-rules, personalities).
- Auto-commit on skill crystallization and reflexion rule generation with semantic commit messages.
- Optional background push to user's private `tau-mind` GitHub repository.

### Phase 5: The Alfred Moral Override Protocol (`crates/pi-core/src/alfred.rs`)
- Implement `AlfredProtocol` monitoring user-stated values and mission integrity.
- Escalation tiers (`Observation`, `Advisory`, `Urgent`, `LastStand`).
- Reflexion-tuned framing that adapts advisory delivery based on historical user receptivity.

### Phase 6: Client Daemon Connectors (`crates/pi-cli` & `crates/pi-tui`)
- Update `tau` CLI with `--daemon`, `tau daemon start`, `tau daemon stop`, `tau daemon status`.
- Auto-detect running `taud.sock` for zero-latency client connections.

---

## Verification Checkpoints

1. **Compilation & Clippy:**
   ```bash
   cargo check --workspace --all-targets
   cargo clippy --workspace --all-targets -- -D warnings
   ```
2. **Test Suite:**
   ```bash
   cargo test --workspace -- --nocapture
   ```
3. **Daemon Smoke Test:**
   - Launch `taud` in background test mode, send Ping request over Unix socket, receive Pong.
   - Dispatch task to `J.A.R.V.I.S.` specialist, verify result written to shared vault.
   - Trigger file mutation, execute `undo`, verify exact byte-for-byte restoration.
