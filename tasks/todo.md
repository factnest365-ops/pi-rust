# Tasks: 100% Pure Rust Daemon (`taud`) & JARVIS Sub-Agent Fleet

- [x] Task 1: Create `crates/pi-daemon` crate with `taud` binary and Unix domain socket IPC
  - Acceptance: `taud` daemon binds to `~/.tau/taud.sock` or fallback path, responds to JSON-RPC 2.0 requests (`ping`, `status`, `eval`, `turn`), and handles clean SIGTERM/SIGINT teardown.
  - Verify: `cargo test -p pi-daemon` passes with e2e socket ping-pong tests.
  - Files: `Cargo.toml`, `crates/pi-daemon/Cargo.toml`, `crates/pi-daemon/src/lib.rs`, `crates/pi-daemon/src/main.rs`, `crates/pi-daemon/src/ipc.rs`

- [x] Task 2: Implement Federated Specialist Fleet (`J.A.R.V.I.S.`, `F.R.I.D.A.Y.`, `E.V.`) in `pi-core`
  - Acceptance: `SpecialistIdentity` enum and `FederatedFleet` manager route tasks to specialized agent personas while sharing unified `TauVault` and `ReflexionEngine`.
  - Verify: Unit tests in `crates/pi-core/src/federation.rs` verifying persona prompt styling, specialization routing, and shared memory access.
  - Files: `crates/pi-core/src/federation.rs`, `crates/pi-core/src/lib.rs`

- [x] Task 3: Implement Full Autonomy Undo & Rollback Engine in `pi-core`
  - Acceptance: `UndoEngine` captures pre/post state snapshots for every file mutation and bash action, supports single/multi-step rollback and split-diff inspection.
  - Verify: Unit tests in `crates/pi-core/src/undo.rs` verifying byte-accurate rollback of edits, file creations, and deletions.
  - Files: `crates/pi-core/src/undo.rs`, `crates/pi-core/src/lib.rs`

- [x] Task 4: Implement Cognitive State Sync & Git Fragmentation in `pi-core`
  - Acceptance: `StateSynchronizer` initializes Git repository in `~/.tau/` (if enabled), auto-commits state changes on crystallization and reflexion, and prepares sync frames.
  - Verify: Unit tests in `crates/pi-core/src/sync.rs` verifying repo initialization and automated commit logging.
  - Files: `crates/pi-core/src/sync.rs`, `crates/pi-core/src/lib.rs`

- [x] Task 5: Implement The Alfred Moral Override Protocol in `pi-core`
  - Acceptance: `AlfredProtocol` tracks user-stated core values, detects value-contradicting actions, and generates non-blocking tiered advisories (`Observation`, `Advisory`, `Urgent`, `LastStand`).
  - Verify: Unit tests in `crates/pi-core/src/alfred.rs` verifying value matching and escalation level transitions.
  - Files: `crates/pi-core/src/alfred.rs`, `crates/pi-core/src/lib.rs`

- [x] Task 6: Integrate Daemon CLI commands in `pi-cli`
  - Acceptance: CLI supports `tau daemon start`, `tau daemon stop`, `tau daemon status`, and automatically connects to running `taud.sock` when available.
  - Verify: `cargo test -p pi-cli` and `cargo check --workspace --all-targets`.
  - Files: `crates/pi-cli/src/main.rs`, `crates/pi-cli/src/lib.rs`
