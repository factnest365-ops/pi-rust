# Factory Rules

**Enforcement status:** These rules are enforced mechanically where marked; others are process constraints observed by the factory loop.

## Non-negotiables (mechanically enforced)
- **Only Rust.** `--stack` must be `rust`; any other value is rejected with no fallback. (enforced in `engine::ForgeOptions::validate`)
- **Zero compiler warnings.** `cargo build --quiet`, `cargo test --quiet`, `cargo clippy --all-targets -- -D warnings` all must pass. (enforced in `stages/crucible.rs`)
- **No hallucinated dependencies.** Only dependencies listed in `manifest.allowed_deps` may appear in the generated `Cargo.toml`; any other key fails the round. (enforced in `deps.rs` + `stages/crucible.rs`)
- **No `unwrap()`/`expect()` outside tests.** Mechanical director rule. (enforced in `director_rules.rs`)
- **No `todo!()`/`unimplemented!()` on shipped paths.** Mechanical director rule. (enforced in `director_rules.rs`)
- **Max 400 lines per source file.** Mechanical director rule. (enforced in `director_rules.rs`)
- **Blueprint is total.** Every tree path owned exactly once; reserved paths only scaffold. (enforced in `manifest::validate`)
- **Isolation.** Destroyers receive contracts only, never forger source. (enforced by agent boundary types)
- **Resumability.** State persisted after every transition to `<out>/.metafuge/run.json`. (enforced in `run.rs`/`engine.rs`)

## Process doctrine (enforced by loop structure)
- **Silence is speed:** workers emit raw code, diffs, and JSON telemetry, not chat. (enforced by agent I/O contract)
- **Adversarial symbiosis:** forgers and destroyers from same contracts. (enforced by pipeline stages)
- **One-shot supremacy:** loop closes automatically with no human prompt. (enforced by engine loop)
- **Bloat is treason:** structural gate rejects bloat/wrapper laziness. (enforced by director rules + gate)

## Output discipline (process)
- **No commented-out dead code, no conversational markdown in source files.** (process rule; not fully parsed by default gates)
- **Agents emit raw code, diffs, and JSON telemetry rather than chat.** (enforced by agent boundary)
