use anyhow::{Context, Result};

const DEPENDENCY_SECTIONS: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

/// One dependency key discovered in the generated `Cargo.toml`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DependencyKey {
    pub section: String,
    pub name: String,
}

/// A dependency the manifest did not authorize.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyViolation {
    pub section: String,
    pub name: String,
}

impl std::fmt::Display for DependencyViolation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "dependency `{}` in [{}] is not in allowed_deps",
            self.name, self.section
        )
    }
}

/// Read every dependency key the generated crate declares.
pub fn dependency_keys(cargo_toml: &str) -> Vec<DependencyKey> {
    let mut keys = Vec::new();
    let mut section = String::new();
    for raw in cargo_toml.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_string();
            if let Some(name) = section.strip_prefix("dependencies.") {
                let name = name.trim().trim_matches('"');
                if !name.is_empty() && !name.contains('.') {
                    keys.push(DependencyKey {
                        section: "dependencies".to_string(),
                        name: name.to_string(),
                    });
                }
            }
            continue;
        }
        if dependency_section(&section).is_none() {
            continue;
        }
        let Some((key, _)) = line.split_once('=') else {
            continue;
        };
        let name = key
            .trim()
            .split('.')
            .next()
            .unwrap_or_default()
            .trim()
            .trim_matches('"');
        if name.is_empty() {
            continue;
        }
        keys.push(DependencyKey {
            section: section.clone(),
            name: name.to_string(),
        });
    }
    keys.dedup();
    keys
}

/// Enforce the allowlist against the generated manifest file.
pub fn check(cargo_toml: &str, allowed: &[String]) -> Vec<DependencyViolation> {
    let mut violations: Vec<DependencyViolation> = Vec::new();
    for key in dependency_keys(cargo_toml) {
        if allowed.iter().any(|candidate| candidate == &key.name) {
            continue;
        }
        let violation = DependencyViolation {
            section: key.section,
            name: key.name,
        };
        if !violations.contains(&violation) {
            violations.push(violation);
        }
    }
    violations
}

/// Parse `[package] name` so callers can prove the scaffold is self-consistent.
pub fn package_name(cargo_toml: &str) -> Option<String> {
    let mut in_package = false;
    for raw in cargo_toml.lines() {
        let line = strip_comment(raw).trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_package = line[1..line.len() - 1].trim() == "package";
            continue;
        }
        if !in_package {
            continue;
        }
        if let Some((key, value)) = line.split_once('=')
            && key.trim() == "name"
        {
            return Some(value.trim().trim_matches('"').to_string());
        }
    }
    None
}

/// Read and check the generated `Cargo.toml`.
pub fn check_file(path: &std::path::Path, allowed: &[String]) -> Result<Vec<DependencyViolation>> {
    let body = std::fs::read_to_string(path)
        .with_context(|| format!("could not read `{}`", path.display()))?;
    Ok(check(&body, allowed))
}

fn dependency_section(section: &str) -> Option<&'static str> {
    let last = section.rsplit('.').next().unwrap_or(section);
    DEPENDENCY_SECTIONS
        .iter()
        .find(|candidate| **candidate == last)
        .copied()
}

fn strip_comment(line: &str) -> &str {
    match line.find('#') {
        Some(index) => &line[..index],
        None => line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#"
[package]
name = "slugify"
version = "0.1.0"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json.workspace = true

[dev-dependencies]
tokio = { version = "1" }

[target.'cfg(unix)'.dependencies]
libc = "0.2"
"#;

    #[test]
    fn reads_all_dependency_sections() {
        let keys = dependency_keys(MANIFEST);
        let names: Vec<&str> = keys.iter().map(|key| key.name.as_str()).collect();
        assert_eq!(names, vec!["serde", "serde_json", "tokio", "libc"]);
    }

    #[test]
    fn allows_authorized_keys_only() {
        let allowed = vec!["serde".to_string(), "serde_json".to_string()];
        let violations = check(MANIFEST, &allowed);
        assert_eq!(violations.len(), 2);
        assert!(violations.iter().any(|v| v.name == "tokio"));
        assert!(violations.iter().any(|v| v.name == "libc"));
    }

    #[test]
    fn an_empty_allowlist_rejects_everything() {
        let violations = check(MANIFEST, &[]);
        assert_eq!(violations.len(), 4);
        assert!(
            violations
                .iter()
                .any(|v| v.to_string().contains("not in allowed_deps"))
        );
    }

    #[test]
    fn dotted_dependency_tables_are_recognized() {
        let body = "[dependencies.serde]\nversion = \"1\"\n";
        let keys = dependency_keys(body);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].name, "serde");
        assert_eq!(keys[0].section, "dependencies");
    }

    #[test]
    fn reads_the_package_name() {
        assert_eq!(package_name(MANIFEST).as_deref(), Some("slugify"));
    }
}
