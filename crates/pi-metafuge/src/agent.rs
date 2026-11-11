use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::director_rules::SourceFile;
use crate::manifest::{ArchitectureBlueprint, DestroyerTask, ForgerTask, Manifest, SharedContract};
use crate::verdict::{JudgeDecision, Violation};

pub mod subprocess;
pub mod template;

pub use subprocess::SubprocessAgent;
pub use template::TemplateAgent;

/// Which worker implementation the run was started with. Persisted so a resume
/// rebuilds the same agent without being told twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    Template,
    Subprocess,
}

impl AgentKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            AgentKind::Template => "template",
            AgentKind::Subprocess => "subprocess",
        }
    }
}

/// Honest description of who produced the code. Printed and serialized so no
/// output can imply a model wrote template output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDescription {
    pub kind: AgentKind,
    pub name: String,
    pub note: String,
    pub model_backed: bool,
}

/// The boundary between the forge loop and whatever writes the code.
///
/// Both sides receive only contracts and task payloads. A destroyer request
/// never carries forger source, which is what makes the test side adversarial.
#[async_trait]
pub trait MetafugeAgent: Send + Sync {
    fn describe(&self) -> AgentDescription;

    async fn spec(&self, request: &SpecRequest) -> Result<SpecResponse>;
    async fn forge(&self, request: &ForgeRequest) -> Result<ForgeResponse>;
    async fn destroy(&self, request: &DestroyRequest) -> Result<DestroyResponse>;
    async fn judge(&self, request: &JudgeRequest) -> Result<JudgeResponse>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedFile {
    pub path: String,
    pub contents: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecRequest {
    pub schema: u32,
    pub prompt: String,
    pub stack: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecResponse {
    pub manifest: Manifest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeRequest {
    pub round: u32,
    pub task: ForgerTask,
    pub contracts: Vec<SharedContract>,
    pub blueprint: ArchitectureBlueprint,
    /// Trimmed compiler or test diagnostics from the previous round only.
    pub diagnostic: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeResponse {
    pub files: Vec<GeneratedFile>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DestroyRequest {
    pub round: u32,
    pub task: DestroyerTask,
    pub contracts: Vec<SharedContract>,
    pub blueprint: ArchitectureBlueprint,
    pub diagnostic: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DestroyResponse {
    pub files: Vec<GeneratedFile>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeRequest {
    pub round: u32,
    pub manifest: Manifest,
    pub source_files: Vec<SourceFile>,
    /// Mechanical violations. When non-empty the engine refuses approval and
    /// never asks the agent.
    pub mechanical_violations: Vec<Violation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeResponse {
    pub decision: JudgeDecision,
    pub violations: Vec<Violation>,
    pub rationale: String,
}
