use std::fmt;

use serde::{Deserialize, Serialize};

/// The two lines the director gate is allowed to emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerdictStatus {
    ApprovedForBareMetal,
    RejectedByOverlord,
}

impl VerdictStatus {
    pub const fn status_line(self) -> &'static str {
        match self {
            VerdictStatus::ApprovedForBareMetal => "STATUS: APPROVED_FOR_BARE_METAL",
            VerdictStatus::RejectedByOverlord => "STATUS: REJECTED_BY_OVERLORD",
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            VerdictStatus::ApprovedForBareMetal => "approved_for_bare_metal",
            VerdictStatus::RejectedByOverlord => "rejected_by_overlord",
        }
    }
}

/// Who produced the verdict. Mechanical rules are never described as model
/// judgment, and a model-backed verdict is labelled as such.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JudgeSource {
    Mechanical,
    Agent,
}

impl JudgeSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            JudgeSource::Mechanical => "mechanical",
            JudgeSource::Agent => "agent",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JudgeDecision {
    Approve,
    Reject,
}

/// A structural defect pinned to a file and line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Violation {
    pub file: String,
    pub line: usize,
    pub rule: String,
    pub message: String,
}

impl Violation {
    pub fn new(
        file: impl Into<String>,
        line: usize,
        rule: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            file: file.into(),
            line,
            rule: rule.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for Violation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{}: [{}] {}",
            self.file, self.line, self.rule, self.message
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Verdict {
    pub status: VerdictStatus,
    pub round: u32,
    pub judged_by: JudgeSource,
    pub violations: Vec<Violation>,
    pub rationale: String,
    pub at: String,
}

impl Verdict {
    pub fn approved(&self) -> bool {
        self.status == VerdictStatus::ApprovedForBareMetal
    }
}
