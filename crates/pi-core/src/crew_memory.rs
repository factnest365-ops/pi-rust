use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::SkillCrystallizer;
use crate::SkillRegistry;
use crate::vault::MemoryEntry;
use crate::vault::TauVault;
use pi_session::SessionNode;

/// Maximum number of skills injected per crew task.
pub const DEFAULT_CREW_SKILL_LIMIT: usize = 5;

/// Provenance record for a crew memory/skill injection event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrewProvenance {
    pub source: String,
    pub event: String,
    pub task_id: Option<String>,
    pub crew_id: Option<String>,
    pub recorded_at: String,
    pub meta: BTreeMap<String, String>,
}

impl CrewProvenance {
    pub fn new(source: &str, event: &str) -> Self {
        Self {
            source: source.to_string(),
            event: event.to_string(),
            task_id: None,
            crew_id: None,
            recorded_at: Utc::now().to_rfc3339(),
            meta: BTreeMap::new(),
        }
    }

    pub fn with_task_id(mut self, task_id: impl Into<String>) -> Self {
        self.task_id = Some(task_id.into());
        self
    }

    pub fn with_crew_id(mut self, crew_id: impl Into<String>) -> Self {
        self.crew_id = Some(crew_id.into());
        self
    }

    pub fn with_meta(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.meta.insert(key.into(), value.into());
        self
    }
}

/// Ledger entry for a selected or crystallized crew skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrewSkillLedgerEntry {
    pub skill_name: String,
    pub description: String,
    pub path: PathBuf,
    pub score: f64,
    pub source: String,
    pub provenance: CrewProvenance,
}

/// Input for crew prefetch before dispatch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrewPrefetchInput {
    pub task_text: String,
    pub crew_id: Option<String>,
    pub task_id: Option<String>,
    pub skill_limit: Option<usize>,
    pub memory_limit: Option<usize>,
}

impl Default for CrewPrefetchInput {
    fn default() -> Self {
        Self {
            task_text: String::new(),
            crew_id: None,
            task_id: None,
            skill_limit: Some(DEFAULT_CREW_SKILL_LIMIT),
            memory_limit: Some(5),
        }
    }
}

/// Result of crew prefetch: selected skills, recalled memories, provenance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrewPrefetchResult {
    pub task_text: String,
    pub crew_id: Option<String>,
    pub task_id: Option<String>,
    pub memories: Vec<MemoryEntry>,
    pub selected_skills: Vec<CrewSkillLedgerEntry>,
    pub provenance: CrewProvenance,
    pub skill_context_md: String,
}

/// Input for post-crew crystallization after a winning trajectory.
#[derive(Debug, Clone)]
pub struct CrewCrystallizationInput {
    pub task_id: String,
    pub crew_id: Option<String>,
    pub skill_name: String,
    pub description: String,
    pub trajectory: Vec<SessionNode>,
    pub vault: Arc<TauVault>,
    pub registry: SkillRegistry,
    pub skill_limit: Option<usize>,
}

/// Record produced after crystallization, suitable for status reporting or downstream storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrewCrystallizationRecord {
    pub task_id: String,
    pub crew_id: Option<String>,
    pub skill_name: String,
    pub skill_path: PathBuf,
    pub vault_memory_id: Option<String>,
    pub provenance: CrewProvenance,
    pub skill_content: String,
}

impl CrewPrefetchResult {
    /// Builds a compact markdown context block for injection into crew prompts.
    pub fn build_skill_context(&self) -> String {
        let mut out = String::new();
        out.push_str("\n--- Crew Memory Recall ---\n");

        if !self.memories.is_empty() {
            out.push_str("## Relevant Memories\n");
            for mem in &self.memories {
                out.push_str(&format!(
                    "- [{}] ({}): {}\n",
                    mem.scope, mem.topic, mem.content
                ));
            }
            out.push('\n');
        }

        if !self.selected_skills.is_empty() {
            out.push_str("## Relevant Skills\n");
            out.push_str(
                "Use only these selected skills. Do not load all available skills.\n",
            );
            for entry in &self.selected_skills {
                out.push_str(&format!(
                    "- {}: {} (`{}`)\n",
                    entry.skill_name, entry.description, entry.path.display()
                ));
            }
            out.push('\n');
        }

        out.push_str("[End Crew Memory Recall]\n");
        out
    }
}

/// Scores registry skills against task text and returns ranked candidates.
pub fn score_skills_for_task(
    registry: &SkillRegistry,
    task_text: &str,
    limit: usize,
) -> Vec<CrewSkillLedgerEntry> {
    if registry.skills.is_empty() || task_text.trim().is_empty() {
        return Vec::new();
    }

    let query = task_text.to_ascii_lowercase();
    let mut scored: Vec<CrewSkillLedgerEntry> = registry
        .skills
        .iter()
        .map(|skill| {
            let haystack = format!(
                "{} {}",
                skill.name.to_ascii_lowercase(),
                skill.description.to_ascii_lowercase()
            );
            let mut score: f64 = 0.0;

            for token in query.split_whitespace() {
                if token.len() < 3 {
                    continue;
                }
                let mut count: usize = 0;
                for part in haystack.split_whitespace() {
                    if part == token {
                        count += 2;
                    } else if part.starts_with(token) || part.ends_with(token) {
                        count += 1;
                    }
                }
                score += count as f64;
            }

            (score, skill)
        })
        .filter_map(|(score, skill)| {
            if score > 0.0 {
                Some(CrewSkillLedgerEntry {
                    skill_name: skill.name.clone(),
                    description: skill.description.clone(),
                    path: skill.path.clone(),
                    score,
                    source: "registry".to_string(),
                    provenance: CrewProvenance::new("score_skills_for_task", "skill_scored"),
                })
            } else {
                None
            }
        })
        .collect();

    scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(limit.max(1));
    scored
}

/// Runs crew prefetch: hybrid vault recall + limited skill selection + context synthesis.
pub fn prefetch_crew_context(
    vault: &TauVault,
    registry: &SkillRegistry,
    input: CrewPrefetchInput,
) -> Result<CrewPrefetchResult> {
    let skill_limit = input.skill_limit.unwrap_or(DEFAULT_CREW_SKILL_LIMIT);
    let memory_limit = input.memory_limit.unwrap_or(5);

    let provenance = CrewProvenance::new("prefetch_crew_context", "crew_prefetch")
        .with_task_id(input.task_id.as_deref().unwrap_or("unknown"))
        .with_crew_id(input.crew_id.as_deref().unwrap_or("unknown"))
        .with_meta("skill_limit", skill_limit.to_string())
        .with_meta("memory_limit", memory_limit.to_string());

    let memories = if input.task_text.trim().is_empty() {
        Vec::new()
    } else {
        vault.search_hybrid(&input.task_text, memory_limit)?
    };

    let selected_skills = score_skills_for_task(registry, &input.task_text, skill_limit);

    let result = CrewPrefetchResult {
        task_text: input.task_text.clone(),
        crew_id: input.crew_id.clone(),
        task_id: input.task_id.clone(),
        memories,
        selected_skills,
        provenance,
        skill_context_md: String::new(),
    };

    Ok(result)
}

/// Crystallizes a winning crew trajectory into a skill, persists to disk, registers in registry,
/// and records an episodic memory + provenance entry in the vault.
pub fn crystallize_crew_outcome(input: CrewCrystallizationInput) -> Result<CrewCrystallizationRecord> {
    let skill_limit = input.skill_limit.unwrap_or(DEFAULT_CREW_SKILL_LIMIT);
    let provenance = CrewProvenance::new("crystallize_crew_outcome", "crew_crystallization")
        .with_task_id(&input.task_id)
        .with_crew_id(input.crew_id.as_deref().unwrap_or("unknown"))
        .with_meta("trajectory_nodes", input.trajectory.len().to_string())
        .with_meta("skill_limit", skill_limit.to_string());

    let session_nodes: Vec<&SessionNode> = input.trajectory.iter().collect();

    let (skill_path, skill_content) =
        SkillCrystallizer::crystallize_and_register_refs(&mut input.registry.clone(), &session_nodes, &input.skill_name, &input.description)?;

    let vault_memory_id = input
        .vault
        .record_episodic_memory(&format!("crew-skill:{}", input.skill_name), &format!(
            "Crystallized crew skill `{}` from task `{}`. Path: {}",
            input.skill_name,
            input.task_id,
            skill_path.display()
        ))
        .ok();

    let provenance = provenance.with_meta("skill_path", skill_path.display().to_string());

    Ok(CrewCrystallizationRecord {
        task_id: input.task_id,
        crew_id: input.crew_id,
        skill_name: input.skill_name,
        skill_path,
        vault_memory_id,
        provenance,
        skill_content,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SkillDefinition;
    use pi_session::Role;

    #[test]
    fn test_provenance_builder() {
        let provenance = CrewProvenance::new("unit-test", "prefetch")
            .with_task_id("task-1")
            .with_crew_id("crew-a")
            .with_meta("key", "value");

        assert_eq!(provenance.source, "unit-test");
        assert_eq!(provenance.event, "prefetch");
        assert_eq!(provenance.task_id, Some("task-1".to_string()));
        assert_eq!(provenance.crew_id, Some("crew-a".to_string()));
        assert_eq!(provenance.meta.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_score_skills_for_task_ranks_and_limits() {
        let mut registry = SkillRegistry::default();
        registry.skills.clear();
        registry.skills.push(SkillDefinition {
            name: "Rust CI Automator".to_string(),
            description: "Sets up GitHub Actions CI with clippy and test coverage".to_string(),
            path: PathBuf::from("skills/rust-ci/SKILL.md"),
            content: String::new(),
        });
        registry.skills.push(SkillDefinition {
            name: "Frontend Polish".to_string(),
            description: "Improves CSS animations and accessibility".to_string(),
            path: PathBuf::from("skills/frontend/SKILL.md"),
            content: String::new(),
        });
        registry.skills.push(SkillDefinition {
            name: "Git Workflow".to_string(),
            description: "Manages branches, rebasing, and commits".to_string(),
            path: PathBuf::from("skills/git/SKILL.md"),
            content: String::new(),
        });

        let scored = score_skills_for_task(&registry, "setup CI with clippy checks", 2);
        assert_eq!(scored.len(), 1);
        assert_eq!(scored[0].skill_name, "Rust CI Automator");
    }

    #[test]
    fn test_prefetch_crew_context_synthesizes_memories_and_skills() {
        let vault = TauVault::open_in_memory().expect("in-memory vault must open");
        vault
            .record_memory("episodic", "crew-rust-ci", "Previous crew shipped CI with clippy", None, None, None)
            .expect("memory recorded");

        let mut registry = SkillRegistry::default();
        registry.skills.push(SkillDefinition {
            name: "Rust CI Automator".to_string(),
            description: "Sets up GitHub Actions CI with clippy".to_string(),
            path: PathBuf::from("skills/rust-ci/SKILL.md"),
            content: String::new(),
        });

        let input = CrewPrefetchInput {
            task_text: "setup CI with clippy checks".to_string(),
            crew_id: Some("crew-1".to_string()),
            task_id: Some("task-1".to_string()),
            skill_limit: Some(2),
            memory_limit: Some(5),
        };

        let result = prefetch_crew_context(&vault, &registry, input).expect("prefetch succeeds");
        assert_eq!(result.crew_id, Some("crew-1".to_string()));
        assert_eq!(result.task_id, Some("task-1".to_string()));
        assert_eq!(result.memories.len(), 1);
        assert_eq!(result.selected_skills.len(), 1);
        assert_eq!(result.selected_skills[0].skill_name, "Rust CI Automator");

        let context = result.build_skill_context();
        assert!(context.contains("## Relevant Memories"));
        assert!(context.contains("## Relevant Skills"));
        assert!(context.contains("[End Crew Memory Recall]"));
    }

    #[test]
    fn test_crystallize_crew_outcome_writes_skill_and_vault_memory() {
        let vault = TauVault::open_in_memory().expect("in-memory vault must open");
        let registry = SkillRegistry::default();

        let nodes = vec![SessionNode {
            id: "u1".to_string(),
            parent_id: None,
            children_ids: Vec::new(),
            role: Role::User,
            content: "Setup automated CI workflow with clippy checks".to_string(),
            timestamp: Utc::now().to_rfc3339(),
            tool_call_id: None,
            tool_name: None,
            tool_calls: None,
        }];

        let input = CrewCrystallizationInput {
            task_id: "task-1".to_string(),
            crew_id: Some("crew-1".to_string()),
            skill_name: "Rust CI Automator".to_string(),
            description: "Sets up GitHub Actions CI with clippy and test coverage".to_string(),
            trajectory: nodes,
            vault: Arc::new(vault),
            registry,
            skill_limit: Some(5),
        };

        let record = crystallize_crew_outcome(input).expect("crystallization succeeds");
        assert_eq!(record.task_id, "task-1");
        assert_eq!(record.crew_id, Some("crew-1".to_string()));
        assert!(record.skill_path.ends_with("rust-ci-automator/SKILL.md"));
        assert!(record.skill_content.contains("# Rust Ci Automator"));
        assert!(record.vault_memory_id.is_some());
        assert_eq!(record.provenance.event, "crew_crystallization");
    }
}
