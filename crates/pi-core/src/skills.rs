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

        let mut search_paths = Vec::new();

        // Project local paths
        search_paths.push(PathBuf::from(".pi/skills"));
        search_paths.push(PathBuf::from(".agents/skills"));
        search_paths.push(PathBuf::from("skills"));

        // Global ~/.pi paths
        if let Some(home) = dirs::home_dir() {
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
            && let Some(end_idx) = rest.find("---")
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
        let mut lines = content.lines();
        let first_heading = lines
            .find(|l| l.starts_with('#'))
            .map(|l| l.trim_start_matches('#').trim().to_string())
            .unwrap_or(folder_name);

        let desc = lines
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
}
