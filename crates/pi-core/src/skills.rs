use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillDefinition {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    pub content: String,
}

#[derive(Debug, Clone, Default)]
pub struct SkillRegistry {
    pub skills: Vec<SkillDefinition>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        let mut registry = Self::default();
        registry.discover_all();
        registry
    }

    pub fn discover_all(&mut self) {
        self.skills.clear();

        let mut search_paths = vec![
            PathBuf::from(".tau/skills"),
            PathBuf::from(".pi/skills"),
            PathBuf::from(".agents/skills"),
            PathBuf::from("skills"),
        ];

        // Global ~/.tau and ~/.pi paths
        if let Some(home) = dirs::home_dir() {
            search_paths.push(home.join(".tau").join("skills"));
            search_paths.push(home.join(".pi").join("agent").join("skills"));
            search_paths.push(home.join(".pi").join("skills"));
        }

        for base_path in search_paths {
            if base_path.exists() && base_path.is_dir() {
                self.scan_directory(&base_path);
            }
        }
    }

    fn scan_directory(&mut self, dir: &Path) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let skill_file = if path.is_dir() {
                path.join("SKILL.md")
            } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
                path.clone()
            } else {
                continue;
            };

            if skill_file.exists() && skill_file.is_file()
                && let Ok(content) = fs::read_to_string(&skill_file)
            {
                let (name, description) = Self::parse_frontmatter_or_fallback(&content, &path);

                // Avoid duplicate skill names
                if !self.skills.iter().any(|s| s.name == name) {
                    self.skills.push(SkillDefinition {
                        name,
                        description,
                        path: skill_file,
                        content,
                    });
                }
            }
        }
    }

    fn parse_frontmatter_or_fallback(content: &str, folder_path: &Path) -> (String, String) {
        let folder_name = folder_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unnamed-skill")
            .to_string();

        let trimmed = content.trim_start();
        if let Some(rest) = trimmed.strip_prefix("---")
            && let Some(end_idx) = rest.find("\n---")
        {
            let frontmatter = &rest[..end_idx];
            let mut name = None;
            let mut description = None;
            let mut in_multiline_desc = false;
            let mut desc_lines = Vec::new();

            for line in frontmatter.lines() {
                let line_trimmed = line.trim();
                if let Some(rest) = line_trimmed.strip_prefix("name:") {
                    in_multiline_desc = false;
                    name = Some(rest.trim().trim_matches('"').trim_matches('\'').to_string());
                } else if let Some(rest) = line_trimmed.strip_prefix("description:") {
                    let d = rest.trim().trim_matches('"').trim_matches('\'');
                    if d == "|" || d == ">" || d.is_empty() {
                        in_multiline_desc = true;
                        desc_lines.clear();
                    } else {
                        in_multiline_desc = false;
                        description = Some(d.to_string());
                    }
                } else if in_multiline_desc {
                    if line.starts_with("  ") || line.starts_with('\t') || (!line_trimmed.is_empty() && !line_trimmed.contains(':')) {
                        desc_lines.push(line_trimmed);
                    } else {
                        in_multiline_desc = false;
                    }
                }
            }

            if description.is_none() && !desc_lines.is_empty() {
                description = Some(desc_lines.join(" "));
            }

            return (
                name.unwrap_or(folder_name),
                description.unwrap_or_else(|| "Custom specialized skill".to_string()),
            );
        }

        // Fallback: Use first heading as name and first line as description
        let first_heading = content
            .lines()
            .find(|l| l.starts_with('#'))
            .map(|l| l.trim_start_matches('#').trim().to_string())
            .unwrap_or(folder_name);

        let desc = content
            .lines()
            .find(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .unwrap_or("Custom specialized skill")
            .trim()
            .to_string();

        (first_heading, desc)
    }

    pub fn format_prompt_summary(&self) -> String {
        if self.skills.is_empty() {
            return String::new();
        }

        let mut out = String::from("\n\n--- Available Skills ---\n");
        out.push_str("You have access to specialized skills:\n");

        for skill in &self.skills {
            out.push_str(&format!("- {}: {}\n", skill.name, skill.description));
        }

        out
    }

    pub fn get_skill(&self, name: &str) -> Option<&SkillDefinition> {
        self.skills.iter().find(|s| s.name.eq_ignore_ascii_case(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter() {
        let markdown = r#"---
name: code-reviewer
description: Senior code reviewer that evaluates changes across five dimensions.
---

# Instructions
Review code carefully.
"#;
        let (name, desc) = SkillRegistry::parse_frontmatter_or_fallback(markdown, Path::new("dummy"));
        assert_eq!(name, "code-reviewer");
        assert_eq!(
            desc,
            "Senior code reviewer that evaluates changes across five dimensions."
        );
    }

    #[test]
    fn test_parse_multiline_frontmatter() {
        let markdown = r#"---
name: "agent-architect"
description: |
  Guides architectural decomposition, crate partitioning,
  and strict decoupling invariants.
---

# Architecture Plan
"#;
        let (name, desc) = SkillRegistry::parse_frontmatter_or_fallback(markdown, Path::new("dummy"));
        assert_eq!(name, "agent-architect");
        assert!(desc.contains("Guides architectural decomposition"));
        assert!(desc.contains("strict decoupling invariants"));
    }

    #[test]
    fn test_parse_fallback_heading_and_body() {
        let markdown = r#"# Git Workflow Specialist

Specialized in branch management, rebasing, and atomic commits.

## Steps
1. Verify branch status.
"#;
        let (name, desc) = SkillRegistry::parse_frontmatter_or_fallback(markdown, Path::new("skills/git-specialist"));
        assert_eq!(name, "Git Workflow Specialist");
        assert_eq!(desc, "Specialized in branch management, rebasing, and atomic commits.");
    }

    #[test]
    fn test_parse_fallback_without_heading() {
        let markdown = "Just raw markdown content without any header.";
        let (name, desc) = SkillRegistry::parse_frontmatter_or_fallback(markdown, Path::new("my-custom-skill"));
        assert_eq!(name, "my-custom-skill");
        assert_eq!(desc, "Just raw markdown content without any header.");
    }

    #[test]
    fn test_skill_registry_formatting_and_lookup() {
        let mut registry = SkillRegistry::default();
        assert_eq!(registry.format_prompt_summary(), "");

        registry.skills.push(SkillDefinition {
            name: "Rust-Optimizer".to_string(),
            description: "Profiles and optimizes Rust code".to_string(),
            path: PathBuf::from("skills/rust-opt/SKILL.md"),
            content: "# Rust Optimizer".to_string(),
        });

        let summary = registry.format_prompt_summary();
        assert!(summary.contains("--- Available Skills ---"));
        assert!(summary.contains("- Rust-Optimizer: Profiles and optimizes Rust code"));

        assert!(registry.get_skill("rust-optimizer").is_some());
        assert!(registry.get_skill("RUST-OPTIMIZER").is_some());
        assert!(registry.get_skill("non-existent").is_none());
    }

    #[test]
    fn test_scan_directory_with_tempdir() {
        let tmp = tempfile::tempdir().unwrap();
        let skill_dir = tmp.path().join("my-skill");
        fs::create_dir_all(&skill_dir).unwrap();

        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: Test-Skill\ndescription: A test skill in temporary directory\n---\n# Content\n",
        )
        .unwrap();

        let mut registry = SkillRegistry::default();
        registry.scan_directory(tmp.path());

        assert_eq!(registry.skills.len(), 1);
        assert_eq!(registry.skills[0].name, "Test-Skill");
        assert_eq!(registry.skills[0].description, "A test skill in temporary directory");
    }
}
