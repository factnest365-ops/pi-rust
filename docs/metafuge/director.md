# Director

**Enforcement:** Mechanical checks run first and are not negotiable (`director_rules.rs`). The agent judgment path is only used after mechanical gates are clean, and all violations are pinned to `file:line` with explicit rule names. The verdict lines are exact and machine-parsable.

## Role
The director gate decides whether the code is merely working or structurally sound. It either approves for bare metal or commands a full refactor.

## Doctrine
- **Bloat is treason.** Unnecessary heap allocation, wrapper bloat, lazy error handling, and cache hostility are rejected.
- **Zero compiler warnings.** Enforced by `cargo clippy --all-targets -- -D warnings`.
- **No hallucinated dependencies.** Enforced by the dependency allowlist parsed from `Cargo.toml`.
- **Silence is speed.** Workers emit raw code, diffs, and JSON telemetry, not conversational markdown.
- **Adversarial symbiosis.** Forgers and destroyers remain isolated; the crucible arbitrates.

## Mechanical rules (enforced)
- No `unwrap()` or `expect()` outside tests.
- No `todo!()` or `unimplemented!()` on shipped paths.
- No source file over 400 lines.
- Dependency keys must be in `manifest.allowed_deps` (all sections, including target-specific and dotted tables).
- Test targets may panic by definition.

## Model-judged items
- Structural soundness beyond the mechanical rules above (e.g., unnecessary heap allocation, wrapper bloat, cache hostility) may be evaluated by a model-backed judge when the seam is used, but in the default template pipeline the agent approval is mechanical/fixture-labeled and no model judgment is required for approval. The system labels the judge source explicitly.
