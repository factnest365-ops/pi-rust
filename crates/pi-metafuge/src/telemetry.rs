use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::support::{self, CommandResult};

pub const TELEMETRY_SCHEMA: u32 = 1;

/// One recorded stage-round, written to
/// `<out>/.metafuge/telemetry/<stage>-<round>.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryRecord {
    pub schema: u32,
    pub run_id: String,
    pub stage: String,
    pub round: u32,
    pub started_at: String,
    pub duration_ms: u64,
    pub ok: bool,
    pub commands: Vec<CommandResult>,
    pub notes: Vec<String>,
}

impl TelemetryRecord {
    pub fn new(run_id: &str, stage: &str, round: u32, started_at: String) -> Self {
        Self {
            schema: TELEMETRY_SCHEMA,
            run_id: run_id.to_string(),
            stage: stage.to_string(),
            round,
            started_at,
            duration_ms: 0,
            ok: false,
            commands: Vec::new(),
            notes: Vec::new(),
        }
    }

    pub fn write(&self, directory: &Path) -> Result<PathBuf> {
        let path = directory.join(format!("{}-{}.json", self.stage, self.round));
        support::write_atomic(&path, &serde_json::to_string_pretty(self)?)?;
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_one_file_per_stage_and_round() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut record = TelemetryRecord::new("run-1", "crucible", 2, support::now());
        record.ok = true;
        record.notes.push("clippy clean".to_string());
        let path = record.write(dir.path()).expect("write");
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("crucible-2.json")
        );
        let raw = std::fs::read_to_string(&path).expect("read");
        let parsed: TelemetryRecord = serde_json::from_str(&raw).expect("parse");
        assert!(parsed.ok);
        assert_eq!(parsed.round, 2);
        assert_eq!(parsed.notes, vec!["clippy clean".to_string()]);
    }
}
