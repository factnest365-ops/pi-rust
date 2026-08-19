use anyhow::{Context, Result};
use pi_session::{Role, SessionNode, SessionTree};
use std::fs;
use std::path::{Path, PathBuf};

use crate::skills::{SkillDefinition, SkillRegistry};

/// Engine for distilling and crystallizing multi-step problem-solving trajectories into reusable skills (`SKILL.md`).
pub struct SkillCrystallizer;

impl SkillCrystallizer {
    /// Converts any freeform string or phrase into a clean lowercase kebab-case identifier.
    /// Example: "Deploy K8s Cluster!" -> "deploy-k8s-cluster"
    pub fn sanitize_skill_name(name: &str) -> String {
        let mut out = String::new();
        let mut last_was_dash = true;

        for c in name.chars() {
            if c.is_alphanumeric() {
                out.push(c.to_ascii_lowercase());
                last_was_dash = false;
            } else if !last_was_dash {
                out.push('-');
                last_was_dash = true;
            }
        }

        let trimmed = out.trim_matches('-');
        if trimmed.is_empty() {
            "custom-skill".to_string()
        } else {
            trimmed.to_string()
        }
    }

    /// Converts a kebab-case or snake_case name into a clean Title Case string.
    /// Example: "deploy-k8s-cluster" -> "Deploy K8s Cluster"
    pub fn titlecase_skill_name(name: &str) -> String {
        let sanitized = Self::sanitize_skill_name(name);
        sanitized
            .split('-')
            .filter(|w| !w.is_empty())
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    Some(first) => {
                        let capitalized = first.to_uppercase().collect::<String>();
                        format!("{}{}", capitalized, chars.as_str())
                    }
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Extracts relevant trigger keywords for skill discovery and prompt matching.
    pub fn extract_triggers(name: &str, description: &str) -> Vec<String> {
        let mut triggers = Vec::new();
        let name_kebab = Self::sanitize_skill_name(name);

        for part in name_kebab.split('-') {
            if part.len() >= 3 && !triggers.contains(&part.to_string()) {
                triggers.push(part.to_string());
            }
        }

        for word in description.split_whitespace() {
            let clean = word
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_ascii_lowercase();
            if clean.len() >= 4
                && !["with", "this", "that", "from", "when", "your", "into", "then", "have", "some", "more"]
                    .contains(&clean.as_str())
                && !triggers.contains(&clean)
            {
                triggers.push(clean);
                if triggers.len() >= 6 {
                    break;
                }
            }
        }

        if triggers.is_empty() {
            triggers.push(name_kebab);
        }

        triggers
    }

    /// Synthesizes a valid `SKILL.md` document from an iterator of `SessionNode` references.
    pub fn crystallize_from_nodes<'a, I>(
        nodes: I,
        skill_name: &str,
        description: &str,
    ) -> Result<String>
    where
        I: IntoIterator<Item = &'a SessionNode>,
    {
        let sanitized_name = Self::sanitize_skill_name(skill_name);
        let title = Self::titlecase_skill_name(skill_name);
        let triggers = Self::extract_triggers(skill_name, description);

        let mut procedure_steps = Vec::new();
        let mut commands_used = Vec::new();

        for node in nodes {
            match node.role {
                Role::User => {
                    let first_line = node.content.lines().next().unwrap_or("").trim();
                    if !first_line.is_empty()
                        && !first_line.starts_with('/')
                        && procedure_steps.len() < 8
                    {
                        procedure_steps.push(format!("Identify objective: {}", first_line));
                    }
                }
                Role::Assistant => {
                    if let Some(ref tool_calls_json) = node.tool_calls
                        && let Some(arr) = tool_calls_json.as_array()
                    {
                        for call in arr {
                            let tool_name = call
                                .get("function")
                                .and_then(|f| f.get("name"))
                                .and_then(|n| n.as_str())
                                .or_else(|| call.get("name").and_then(|n| n.as_str()))
                                .unwrap_or("");

                            let args_str = call
                                .get("function")
                                .and_then(|f| f.get("arguments"))
                                .and_then(|a| a.as_str())
                                .unwrap_or("");

                            match tool_name {
                                "bash" => {
                                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(args_str)
                                        && let Some(cmd) = parsed.get("command").and_then(|c| c.as_str())
                                    {
                                        let trimmed_cmd = cmd.trim();
                                        if !trimmed_cmd.is_empty() && !commands_used.contains(&trimmed_cmd.to_string()) {
                                            commands_used.push(trimmed_cmd.to_string());
                                            procedure_steps.push(format!("Run command: `{}`", trimmed_cmd));
                                        }
                                    }
                                }
                                "write" | "edit" => {
                                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(args_str)
                                        && let Some(path) = parsed.get("path").and_then(|p| p.as_str())
                                    {
                                        procedure_steps.push(format!("Apply modifications to `{}`", path));
                                    }
                                }
                                "grep" | "find" | "ls" => {
                                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(args_str)
                                        && let Some(pattern) = parsed.get("pattern").and_then(|p| p.as_str())
                                    {
                                        procedure_steps.push(format!("Search workspace for `{}`", pattern));
                                    }
                                }
                                "git" => {
                                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(args_str)
                                        && let Some(action) = parsed.get("action").and_then(|a| a.as_str())
                                    {
                                        procedure_steps.push(format!("Execute git action: `{}`", action));
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Role::Tool => {}
                Role::System => {}
            }
        }

        // Deduplicate adjacent steps
        procedure_steps.dedup();

        if procedure_steps.is_empty() {
            procedure_steps.push("Analyze requirements and inspect workspace files.".to_string());
            procedure_steps.push("Execute necessary code modifications or commands.".to_string());
            procedure_steps.push("Run verification tests to confirm functionality.".to_string());
        }

        if commands_used.is_empty() {
            commands_used.push("# Example verification commands\ncargo check\ncargo test".to_string());
        }

        let triggers_json = serde_json::to_string(&triggers).unwrap_or_else(|_| "[]".to_string());

        let mut doc = String::new();
        doc.push_str("---\n");
        doc.push_str(&format!("name: {}\n", sanitized_name));
        doc.push_str(&format!("description: {}\n", description.trim()));
        doc.push_str("version: 1.0.0\n");
        doc.push_str(&format!("triggers: {}\n", triggers_json));
        doc.push_str("---\n\n");

        doc.push_str(&format!("# {}\n\n", title));
        doc.push_str("## Purpose\n");
        doc.push_str(&format!("{}\n\n", description.trim()));

        doc.push_str("## Step-by-Step Procedure\n");
        for (i, step) in procedure_steps.iter().enumerate() {
            doc.push_str(&format!("{}. {}\n", i + 1, step));
        }
        doc.push('\n');

        doc.push_str("## Code / Command Templates\n");
        doc.push_str("```bash\n");
        for cmd in &commands_used {
            doc.push_str(cmd);
            doc.push('\n');
        }
        doc.push_str("```\n");

        Ok(doc)
    }

    /// Synthesizes a valid `SKILL.md` document from a slice of `SessionNode`.
    pub fn crystallize_from_trajectory(
        trajectory: &[SessionNode],
        skill_name: &str,
        description: &str,
    ) -> Result<String> {
        Self::crystallize_from_nodes(trajectory.iter(), skill_name, description)
    }

    /// Synthesizes a valid `SKILL.md` document from a slice of `SessionNode` references.
    pub fn crystallize_from_ref_trajectory(
        trajectory: &[&SessionNode],
        skill_name: &str,
        description: &str,
    ) -> Result<String> {
        Self::crystallize_from_nodes(trajectory.iter().copied(), skill_name, description)
    }

    /// Distills active session history directly from a `SessionTree`.
    pub fn crystallize_from_session(
        session: &SessionTree,
        skill_name: &str,
        description: &str,
    ) -> Result<String> {
        let history = session.get_active_branch_history();
        Self::crystallize_from_ref_trajectory(&history, skill_name, description)
    }

    /// Saves the synthesized `SKILL.md` content to `~/.tau/skills/<name>/SKILL.md`.
    pub fn save_skill_to_disk(skill_name: &str, content: &str) -> Result<PathBuf> {
        let sanitized = Self::sanitize_skill_name(skill_name);
        let base_dir = dirs::home_dir()
            .map(|h| h.join(".tau").join("skills"))
            .unwrap_or_else(|| PathBuf::from(".tau/skills"));

        Self::save_skill_to_custom_dir(&base_dir, &sanitized, content)
    }

    /// Saves skill markdown to a specified root directory: `<base_dir>/<sanitized_name>/SKILL.md`.
    pub fn save_skill_to_custom_dir(
        base_dir: &Path,
        skill_name: &str,
        content: &str,
    ) -> Result<PathBuf> {
        let sanitized = Self::sanitize_skill_name(skill_name);
        let skill_dir = base_dir.join(&sanitized);
        fs::create_dir_all(&skill_dir)
            .with_context(|| format!("Failed to create skill directory: {:?}", skill_dir))?;

        let file_path = skill_dir.join("SKILL.md");
        fs::write(&file_path, content)
            .with_context(|| format!("Failed to write skill file: {:?}", file_path))?;

        Ok(file_path)
    }

    /// Automatically registers or updates the newly created skill in the in-memory `SkillRegistry`.
    pub fn register_skill(
        registry: &mut SkillRegistry,
        skill_name: &str,
        description: &str,
        content: &str,
        path: PathBuf,
    ) {
        let sanitized = Self::sanitize_skill_name(skill_name);
        if let Some(existing) = registry
            .skills
            .iter_mut()
            .find(|s| s.name.eq_ignore_ascii_case(&sanitized) || s.name.eq_ignore_ascii_case(skill_name))
        {
            existing.description = description.to_string();
            existing.content = content.to_string();
            existing.path = path;
        } else {
            registry.skills.push(SkillDefinition {
                name: sanitized,
                description: description.to_string(),
                path,
                content: content.to_string(),
            });
        }
    }

    /// Synthesizes, writes to disk, and immediately registers a new skill into the `SkillRegistry` from a slice of `SessionNode`.
    pub fn crystallize_and_register(
        registry: &mut SkillRegistry,
        trajectory: &[SessionNode],
        skill_name: &str,
        description: &str,
    ) -> Result<(PathBuf, String)> {
        let content = Self::crystallize_from_trajectory(trajectory, skill_name, description)?;
        let path = Self::save_skill_to_disk(skill_name, &content)?;
        Self::register_skill(registry, skill_name, description, &content, path.clone());
        Ok((path, content))
    }

    /// Synthesizes, writes to disk, and immediately registers a new skill into the `SkillRegistry` from a slice of `SessionNode` references.
    pub fn crystallize_and_register_refs(
        registry: &mut SkillRegistry,
        trajectory: &[&SessionNode],
        skill_name: &str,
        description: &str,
    ) -> Result<(PathBuf, String)> {
        let content = Self::crystallize_from_ref_trajectory(trajectory, skill_name, description)?;
        let path = Self::save_skill_to_disk(skill_name, &content)?;
        Self::register_skill(registry, skill_name, description, &content, path.clone());
        Ok((path, content))
    }

    /// Manually creates a `SKILL.md` from explicit steps and commands.
    pub fn distill_manual(
        name: &str,
        description: &str,
        steps: &[String],
        commands: &[String],
    ) -> String {
        let sanitized_name = Self::sanitize_skill_name(name);
        let title = Self::titlecase_skill_name(name);
        let triggers = Self::extract_triggers(name, description);
        let triggers_json = serde_json::to_string(&triggers).unwrap_or_else(|_| "[]".to_string());

        let mut doc = String::new();
        doc.push_str("---\n");
        doc.push_str(&format!("name: {}\n", sanitized_name));
        doc.push_str(&format!("description: {}\n", description.trim()));
        doc.push_str("version: 1.0.0\n");
        doc.push_str(&format!("triggers: {}\n", triggers_json));
        doc.push_str("---\n\n");

        doc.push_str(&format!("# {}\n\n", title));
        doc.push_str("## Purpose\n");
        doc.push_str(&format!("{}\n\n", description.trim()));

        doc.push_str("## Step-by-Step Procedure\n");
        for (i, step) in steps.iter().enumerate() {
            doc.push_str(&format!("{}. {}\n", i + 1, step));
        }
        doc.push('\n');

        if !commands.is_empty() {
            doc.push_str("## Code / Command Templates\n");
            doc.push_str("```bash\n");
            for cmd in commands {
                doc.push_str(cmd);
                doc.push('\n');
            }
            doc.push_str("```\n");
        }

        doc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_and_titlecase_skill_name() {
        assert_eq!(
            SkillCrystallizer::sanitize_skill_name("Deploy K8s Cluster!"),
            "deploy-k8s-cluster"
        );
        assert_eq!(
            SkillCrystallizer::sanitize_skill_name("  rust_async_profiler  "),
            "rust-async-profiler"
        );
        assert_eq!(
            SkillCrystallizer::titlecase_skill_name("deploy-k8s-cluster"),
            "Deploy K8s Cluster"
        );
        assert_eq!(
            SkillCrystallizer::titlecase_skill_name("rust-async-profiler"),
            "Rust Async Profiler"
        );
    }

    #[test]
    fn test_extract_triggers() {
        let triggers = SkillCrystallizer::extract_triggers(
            "rust-optimizer",
            "Profiles memory allocations and optimizes hot loops in Rust code",
        );
        assert!(triggers.contains(&"rust".to_string()));
        assert!(triggers.contains(&"optimizer".to_string()));
        assert!(triggers.contains(&"profiles".to_string()));
    }

    #[test]
    fn test_crystallize_from_trajectory_with_tool_calls() {
        let mut nodes = Vec::new();

        let u1 = SessionNode {
            id: "user-1".to_string(),
            parent_id: None,
            children_ids: vec!["asst-1".to_string()],
            role: Role::User,
            content: "Setup automated CI workflow with clippy checks".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            tool_call_id: None,
            tool_name: None,
            tool_calls: None,
        };
        nodes.push(u1);

        let mut a1 = SessionNode {
            id: "asst-1".to_string(),
            parent_id: Some("user-1".to_string()),
            children_ids: Vec::new(),
            role: Role::Assistant,
            content: "Executing bash checks".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            tool_call_id: None,
            tool_name: None,
            tool_calls: None,
        };
        a1.tool_calls = Some(serde_json::json!([
            {
                "id": "call_1",
                "type": "function",
                "function": {
                    "name": "bash",
                    "arguments": "{\"command\": \"cargo clippy --workspace --all-targets -- -D warnings\"}"
                }
            },
            {
                "id": "call_2",
                "type": "function",
                "function": {
                    "name": "write",
                    "arguments": "{\"path\": \".github/workflows/ci.yml\", \"content\": \"...\"}"
                }
            }
        ]));
        nodes.push(a1);

        let skill_md = SkillCrystallizer::crystallize_from_trajectory(
            &nodes,
            "Rust CI Automator",
            "Sets up GitHub Actions CI with clippy and test coverage",
        )
        .unwrap();

        assert!(skill_md.starts_with("---\nname: rust-ci-automator\n"));
        assert!(skill_md.contains("description: Sets up GitHub Actions CI with clippy and test coverage"));
        assert!(skill_md.contains("# Rust Ci Automator"));
        assert!(skill_md.contains("## Purpose"));
        assert!(skill_md.contains("## Step-by-Step Procedure"));
        assert!(skill_md.contains("Run command: `cargo clippy --workspace --all-targets -- -D warnings`"));
        assert!(skill_md.contains("Apply modifications to `.github/workflows/ci.yml`"));
        assert!(skill_md.contains("## Code / Command Templates"));
        assert!(skill_md.contains("cargo clippy --workspace --all-targets -- -D warnings"));
    }

    #[test]
    fn test_save_and_register_skill_custom_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let mut registry = SkillRegistry::default();

        let trajectory = vec![
            SessionNode {
                id: "u1".to_string(),
                parent_id: None,
                children_ids: Vec::new(),
                role: Role::User,
                content: "Optimize query speed".to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                tool_call_id: None,
                tool_name: None,
                tool_calls: None,
            },
        ];

        let content = SkillCrystallizer::crystallize_from_trajectory(
            &trajectory,
            "SQL Query Tuner",
            "Analyzes EXPLAIN output and adds missing indexes",
        )
        .unwrap();

        let path = SkillCrystallizer::save_skill_to_custom_dir(tmp.path(), "SQL Query Tuner", &content).unwrap();
        assert!(path.exists());
        assert!(path.ends_with("sql-query-tuner/SKILL.md"));

        SkillCrystallizer::register_skill(
            &mut registry,
            "SQL Query Tuner",
            "Analyzes EXPLAIN output and adds missing indexes",
            &content,
            path,
        );

        assert_eq!(registry.skills.len(), 1);
        assert_eq!(registry.skills[0].name, "sql-query-tuner");
        assert!(registry.get_skill("sql-query-tuner").is_some());
    }

    #[test]
    fn test_distill_manual() {
        let steps = vec![
            "Step 1: Check git status".to_string(),
            "Step 2: Rebase on main".to_string(),
        ];
        let commands = vec!["git status".to_string(), "git rebase origin/main".to_string()];

        let md = SkillCrystallizer::distill_manual(
            "Git Rebase Flow",
            "Cleanly rebases feature branch on main",
            &steps,
            &commands,
        );

        assert!(md.contains("name: git-rebase-flow"));
        assert!(md.contains("# Git Rebase Flow"));
        assert!(md.contains("1. Step 1: Check git status"));
        assert!(md.contains("2. Step 2: Rebase on main"));
        assert!(md.contains("git rebase origin/main"));
    }
}
