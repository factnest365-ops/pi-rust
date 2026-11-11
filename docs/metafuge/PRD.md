# METAFUGE PRD

METAFUGE turns one prompt into a Rust repository that compiles, passes adversarial
tests, clears a structural gate, and can be sealed with a local commit. The user
states the idea once; the loop closes without a human in the middle.

> Enforcement status in this ship: every gate below is mechanically enforced by
> code in `crates/pi-metafuge`. No model judgment is part of the default pipeline.
> The `--agent subprocess` seam can add a model-backed worker, and its output is
> labelled as such; it is not wired to any provider in this ship.

## Problem

A prompt-to-code session normally ends with a human reading logs, copying errors
back into chat, and deciding when "good enough" means stop. That is a chat loop
with a compiler attached. METAFUGE replaces it with a typed state machine whose
transitions are persisted and whose exit conditions are mechanical.

## User and contract

The operator runs one command:

```text
metafuge forge --prompt <text> --out <dir> [--stack rust] [--max-rounds <n>]
               [--agent template|subprocess] [--agent-cmd <cmd>] [--seal] [--json]
metafuge resume --out <dir> [--json]
```

Guarantees:

- Only `rust` is accepted. An unsupported stack is a hard error with no fallback.
- `--out` must be new or empty; an existing `.metafuge/run.json` must be resumed,
  never overwritten.
- After every transition, the whole state is written atomically to
  `<out>/.metafuge/run.json`.
- `resume` continues from the recorded stage with the recorded agent.
- On a failed run the process exits non-zero and the last line of the error is
  the telemetry directory.
- `--json` emits exactly one JSON object on stdout.

## Pipeline

| Stage | Module | What it does | Exit condition |
| --- | --- | --- | --- |
| Spec | `stages/spec.rs` | Agent returns a manifest; `Manifest::validate` proves the blueprint is total | Valid manifest, tasks registered |
| Scaffold | `stages/scaffold.rs` | Writes `Cargo.toml`, `.gitignore`, blueprint directories, manifest mirror | Files on disk |
| Forge | `stages/forge.rs` | Issues only pending forger tasks, each with contracts and trimmed diagnostics | Every forger task complete |
| Destroy | `stages/destroy.rs` | Issues pending destroyer tasks from contracts only, never from forger source | Every destroyer task complete |
| Crucible | `stages/crucible.rs` | Dependency allowlist, then `cargo build`, `cargo test`, `cargo clippy --all-targets -- -D warnings` | All gates pass |
| Director | `stages/director.rs` | Mechanical structural rules, then optional agent judgment | `STATUS: APPROVED_FOR_BARE_METAL` |
| Seal | `stages/seal.rs` | Only with `--seal` and approval: `git init` if needed, stage, one commit | Commit recorded |

`Stage` order is defined once in `run.rs`; the engine dispatches through
`stages::dispatch` and owns the round cap.

## Manifest schema

`schema` is `1`. The manifest is the only architecture input the workers receive.

```json
{
  "schema": 1,
  "title": "slugify",
  "summary": "Deterministic slug generation over arbitrary text.",
  "architecture_blueprint": {
    "tree": ["Cargo.toml", ".gitignore", "src/lib.rs", "tests/integration.rs"],
    "data_flow": "input -> normalize -> join"
  },
  "shared_contracts": [
    { "path": "src/lib.rs", "symbol": "slugify", "signature": "pub fn slugify(input: &str) -> String" }
  ],
  "forger_tasks": [
    {
      "id": "f1",
      "files": ["src/lib.rs"],
      "contract_refs": ["slugify"],
      "spec": "Crate root and contract re-export.",
      "verify": ["cargo build --quiet"]
    }
  ],
  "destroyer_tasks": [
    {
      "id": "d1",
      "targets": ["slugify"],
      "kind": "integration",
      "spec": "Representative sentences and idempotence.",
      "files": ["tests/integration.rs"]
    }
  ],
  "allowed_deps": []
}
```

Validation, mechanically enforced by `Manifest::validate`:

- `schema` must be 1; title, summary, tree, data flow, contracts, and both task
  lists must be non-empty.
- Every tree path is relative and normalized, appears once, and is owned by
  exactly one worker task. `Cargo.toml` and `.gitignore` are reserved to the
  scaffold and may not be claimed.
- Every contract lives at a tree path, has a unique symbol, and its signature
  begins with `pub `.
- Every forger contract reference and every destroyer target resolves to a
  declared contract.
- Every dependency name is a plausible crate name; the allowlist is the only
  source of dependency lines in the generated `Cargo.toml`.

## Repair loop

The crucible is the only stage that triggers the loop, and the director uses the
same mechanism for structural rejection.

- Diagnostics are trimmed to error lines plus two lines of context, capped at 40
  lines (`diagnostics::trim`). A worker never receives the full log.
- A compile error maps to the owner of the file named in the log. A failing
  assertion maps to the forgers whose contracts intersect the failing destroyer's
  targets, because the test is the spec. An unmappable failure re-opens every
  forger.
- Only the mapped tasks return to `pending`; complete tasks are not re-issued.
- The engine increments the round after a retry, goes back to Forge, and fails the
  run when `--max-rounds` is exhausted. Default is 3; round 1 is the first build.

## Telemetry

Each crucible round writes `<out>/.metafuge/telemetry/crucible-<round>.json`:
run id, stage, round, timestamps, duration, an `ok` flag, and every command with
its exit code, stdout, stderr, duration, and timeout flag. Paths are also recorded
in `run.json`.

## Director

The gate emits exactly one of:

```text
STATUS: APPROVED_FOR_BARE_METAL
STATUS: REJECTED_BY_OVERLORD
```

A rejection carries `file:line` violations. Mechanical rejections are final for
the round: the agent is never asked to override them. See `director.md`.

## Seal

Seal runs only when `--seal` was passed and the director approved. It initializes
a local repository if absent, stages everything, and makes exactly one commit
whose message is derived from the manifest title and round count. It never
pushes, tags, or configures a remote. `.metafuge/` is ignored by the generated
`.gitignore`.

## Agents

- `--agent template` (default): deterministic, offline, model-free. It emits a
  fixed `slugify` crate so the entire pipeline is provable in CI. Every description
  says no model wrote the code.
- `--agent subprocess`: runs `--agent-cmd` through the shell, sends one JSON
  envelope on stdin, reads one JSON envelope from stdout, and enforces a timeout.
  Malformed or mismatched output is a clear error. This is the seam for a real
  harness; provider wiring is intentionally out of scope.

## Non-goals

- Containers or sandboxes.
- Non-Rust stacks.
- Daemon, TUI, or RPC integration.
- Real provider calls inside this crate.
- Auto-merge, pushing, tagging, or release publishing.
- Model judgment required for approval.
