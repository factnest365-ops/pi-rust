//! METAFUGE: a one-shot forge loop that turns a vague prompt into a compiled,
//! tested, and structurally gated repository.
//!
//! The loop is a typed state machine: spec, scaffold, forge, destroy, crucible,
//! director, seal. Every transition is persisted to `<out>/.metafuge/run.json`
//! so a run can be resumed without a human in the loop.
//!
//! Doctrine:
//!
//! - Silence is speed: workers emit code, diffs, and JSON telemetry, not chat.
//! - Adversarial symbiosis: forgers write code from shared contracts, destroyers
//!   write adversarial tests from the same contracts without seeing the code.
//! - One-shot supremacy: the user states the idea once and the loop closes.
//! - Bloat is treason: the director gate rejects structurally weak output.
//!
//! The crate ships two agents. The template agent is a deterministic, offline
//! fixture that makes the whole pipeline provable in CI; no model writes its
//! code. The subprocess agent is the seam for a real harness and is not wired
//! to a provider in this ship.

pub mod agent;
pub mod cli;
pub mod deps;
pub mod diagnostics;
pub mod director_rules;
pub mod engine;
pub mod manifest;
pub mod run;
pub mod stages;
pub mod verdict;

mod support;
mod telemetry;

#[cfg(test)]
mod testsupport;

pub use agent::{
    AgentDescription, AgentKind, DestroyRequest, DestroyResponse, ForgeRequest, ForgeResponse,
    GeneratedFile, JudgeRequest, JudgeResponse, MetafugeAgent, SpecRequest, SpecResponse,
};
pub use engine::{Engine, ForgeOptions, RunReport};
pub use manifest::{
    ArchitectureBlueprint, DestroyerKind, DestroyerTask, ForgerTask, MANIFEST_SCHEMA, Manifest,
    SharedContract,
};
pub use run::{
    AgentState, Progress, Run, RunState, RunStatus, Stage, TaskOwner, TaskState, TaskStatus,
};
pub use verdict::{JudgeDecision, JudgeSource, Verdict, VerdictStatus, Violation};
