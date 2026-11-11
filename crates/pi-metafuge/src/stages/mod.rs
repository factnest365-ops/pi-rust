use anyhow::Result;

use crate::run::{Run, Stage};

pub mod crucible;
pub mod destroy;
pub mod director;
pub mod forge;
pub mod scaffold;
pub mod seal;
pub mod spec;

/// What a stage tells the engine to do next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageOutcome {
    /// The stage is done; continue at this stage.
    Advance(Stage),
    /// Repair round: the engine increments the round and sends the run back to
    /// the forge. The engine owns the round cap.
    Retry,
    /// Terminal success.
    Complete,
    /// Terminal failure that a repair round cannot fix.
    Fail(String),
}

/// Dispatch one stage. Every stage shares the same contract:
/// `fn run(&mut Run) -> Result<StageOutcome>`.
pub async fn dispatch(stage: Stage, run: &mut Run) -> Result<StageOutcome> {
    match stage {
        Stage::Spec => spec::run(run).await,
        Stage::Scaffold => scaffold::run(run).await,
        Stage::Forge => forge::run(run).await,
        Stage::Destroy => destroy::run(run).await,
        Stage::Crucible => crucible::run(run).await,
        Stage::Director => director::run(run).await,
        Stage::Seal => seal::run(run).await,
        Stage::Done => Ok(StageOutcome::Complete),
    }
}
