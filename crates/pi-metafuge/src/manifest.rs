use std::collections::BTreeSet;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

/// The only manifest schema this build understands.
pub const MANIFEST_SCHEMA: u32 = 1;

/// Files the scaffold owns; they are not claimed by any worker task.
pub const RESERVED_PATHS: [&str; 2] = ["Cargo.toml", ".gitignore"];

/// The chief-of-staff artifact: one architecture, one contract set, isolated
/// payloads for both sides of the adversarial split.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub schema: u32,
    pub title: String,
    pub summary: String,
    pub architecture_blueprint: ArchitectureBlueprint,
    pub shared_contracts: Vec<SharedContract>,
    pub forger_tasks: Vec<ForgerTask>,
    pub destroyer_tasks: Vec<DestroyerTask>,
    pub allowed_deps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureBlueprint {
    pub tree: Vec<String>,
    pub data_flow: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedContract {
    pub path: String,
    pub symbol: String,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgerTask {
    pub id: String,
    pub files: Vec<String>,
    pub contract_refs: Vec<String>,
    pub spec: String,
    pub verify: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DestroyerTask {
    pub id: String,
    pub targets: Vec<String>,
    pub kind: DestroyerKind,
    pub spec: String,
    /// Optional explicit test path list. Empty means `tests/<kind>.rs`.
    #[serde(default)]
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DestroyerKind {
    Integration,
    Fuzz,
    Boundary,
}

impl DestroyerKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            DestroyerKind::Integration => "integration",
            DestroyerKind::Fuzz => "fuzz",
            DestroyerKind::Boundary => "boundary",
        }
    }

    pub fn default_file(self) -> String {
        format!("tests/{}.rs", self.as_str())
    }
}

impl DestroyerTask {
    pub fn target_files(&self) -> Vec<String> {
        if self.files.is_empty() {
            vec![self.kind.default_file()]
        } else {
            self.files.clone()
        }
    }
}

impl Manifest {
    /// Validate the architecture contract mechanically.
    ///
    /// The blueprint has to be total: every path in the tree is owned by exactly
    /// one worker task, every contract is referenced, and no task can write
    /// outside the tree. A manifest that fails here never reaches the forge.
    pub fn validate(&self) -> Result<()> {
        if self.schema != MANIFEST_SCHEMA {
            bail!(
                "unsupported manifest schema {} (this build accepts {MANIFEST_SCHEMA})",
                self.schema
            );
        }
        if self.title.trim().is_empty() {
            bail!("manifest title is empty");
        }
        if self.architecture_blueprint.tree.is_empty() {
            bail!("architecture blueprint tree is empty");
        }
        if self.architecture_blueprint.data_flow.trim().is_empty() {
            bail!("architecture blueprint data_flow is empty");
        }
        if self.shared_contracts.is_empty() {
            bail!("manifest declares no shared contracts");
        }
        if self.forger_tasks.is_empty() {
            bail!("manifest declares no forger tasks");
        }
        if self.destroyer_tasks.is_empty() {
            bail!("manifest declares no destroyer tasks");
        }

        let mut tree = BTreeSet::new();
        for path in &self.architecture_blueprint.tree {
            validate_relative(path, "architecture_blueprint.tree")?;
            if !tree.insert(path.as_str()) {
                bail!("architecture blueprint lists `{path}` twice");
            }
        }
        if !tree.contains("Cargo.toml") {
            bail!("architecture blueprint must contain `Cargo.toml`");
        }

        let mut symbols = BTreeSet::new();
        for contract in &self.shared_contracts {
            validate_relative(&contract.path, "shared_contracts.path")?;
            if !tree.contains(contract.path.as_str()) {
                bail!(
                    "contract `{}` lives at `{}` which is not in the blueprint tree",
                    contract.symbol,
                    contract.path
                );
            }
            if contract.symbol.trim().is_empty() {
                bail!("contract at `{}` has an empty symbol", contract.path);
            }
            if !symbols.insert(contract.symbol.as_str()) {
                bail!("contract symbol `{}` is declared twice", contract.symbol);
            }
            if !contract.signature.trim_start().starts_with("pub ") {
                bail!(
                    "contract `{}` signature must begin with `pub `",
                    contract.symbol
                );
            }
        }

        let mut owned: BTreeSet<String> = RESERVED_PATHS
            .iter()
            .map(|path| (*path).to_string())
            .collect();
        let mut task_ids = BTreeSet::new();
        for task in &self.forger_tasks {
            if task.id.trim().is_empty() {
                bail!("forger task with empty id");
            }
            if !task_ids.insert(task.id.as_str()) {
                bail!("task id `{}` is declared twice", task.id);
            }
            if task.files.is_empty() {
                bail!("forger task `{}` owns no files", task.id);
            }
            if task.spec.trim().is_empty() {
                bail!("forger task `{}` has an empty spec", task.id);
            }
            for file in &task.files {
                validate_relative(file, "forger_tasks.files")?;
                if !tree.contains(file.as_str()) {
                    bail!(
                        "forger task `{}` owns `{file}` which is not in the blueprint tree",
                        task.id
                    );
                }
                if !owned.insert(file.clone()) {
                    bail!(
                        "path `{file}` is owned twice (forger task `{}` claims a reserved or already owned path)",
                        task.id
                    );
                }
            }
            if task.contract_refs.is_empty() {
                bail!("forger task `{}` references no contracts", task.id);
            }
            for reference in &task.contract_refs {
                if !symbols.contains(reference.as_str()) {
                    bail!(
                        "forger task `{}` references unknown contract `{reference}`",
                        task.id
                    );
                }
            }
        }

        for task in &self.destroyer_tasks {
            if task.id.trim().is_empty() {
                bail!("destroyer task with empty id");
            }
            if !task_ids.insert(task.id.as_str()) {
                bail!("task id `{}` is declared twice", task.id);
            }
            if task.targets.is_empty() {
                bail!("destroyer task `{}` targets no symbols", task.id);
            }
            if task.spec.trim().is_empty() {
                bail!("destroyer task `{}` has an empty spec", task.id);
            }
            for target in &task.targets {
                if !symbols.contains(target.as_str()) {
                    bail!(
                        "destroyer task `{}` targets unknown symbol `{target}`",
                        task.id
                    );
                }
            }
            for file in task.target_files() {
                validate_relative(&file, "destroyer_tasks.files")?;
                if !tree.contains(file.as_str()) {
                    bail!(
                        "destroyer task `{}` writes `{file}` which is not in the blueprint tree",
                        task.id
                    );
                }
                if !owned.insert(file.clone()) {
                    bail!(
                        "path `{file}` is owned twice (destroyer task `{}` claims an already owned path)",
                        task.id
                    );
                }
            }
        }

        for path in &self.architecture_blueprint.tree {
            if !owned.contains(path.as_str()) {
                bail!("blueprint path `{path}` is not owned by any worker task");
            }
        }

        for dep in &self.allowed_deps {
            validate_dependency_name(dep)?;
        }

        Ok(())
    }

    /// Package name rendered into the generated `Cargo.toml`.
    pub fn package_name(&self) -> String {
        let mut name = String::with_capacity(self.title.len());
        for ch in self.title.chars() {
            if ch.is_ascii_alphanumeric() {
                name.push(ch.to_ascii_lowercase());
            } else if !name.is_empty() && !name.ends_with('-') {
                name.push('-');
            }
        }
        let trimmed = name.trim_matches('-');
        if trimmed.is_empty() {
            "metafuge-app".to_string()
        } else {
            trimmed.to_string()
        }
    }

    pub fn total_tasks(&self) -> usize {
        self.forger_tasks.len() + self.destroyer_tasks.len()
    }

    /// Resolve the owning task for a path in the blueprint.
    pub fn owner_of(&self, path: &str) -> Option<(&str, &'static str)> {
        let path = path.trim_start_matches("./");
        for task in &self.forger_tasks {
            if task.files.iter().any(|file| file == path) {
                return Some((task.id.as_str(), "forger"));
            }
        }
        for task in &self.destroyer_tasks {
            if task.target_files().iter().any(|file| file == path) {
                return Some((task.id.as_str(), "destroyer"));
            }
        }
        None
    }

    pub fn forger_task(&self, id: &str) -> Option<&ForgerTask> {
        self.forger_tasks.iter().find(|task| task.id == id)
    }

    pub fn destroyer_task(&self, id: &str) -> Option<&DestroyerTask> {
        self.destroyer_tasks.iter().find(|task| task.id == id)
    }
}

fn validate_relative(path: &str, field: &str) -> Result<()> {
    if path.trim().is_empty() {
        bail!("{field}: empty path");
    }
    if path.starts_with('/') {
        bail!("{field}: `{path}` is absolute");
    }
    if path.contains('\\') {
        bail!("{field}: `{path}` uses a backslash separator");
    }
    if path
        .split('/')
        .any(|segment| segment == ".." || segment.is_empty())
    {
        bail!("{field}: `{path}` escapes the target directory");
    }
    if path.split('/').any(|segment| segment == ".") {
        bail!("{field}: `{path}` is not normalized");
    }
    Ok(())
}

fn validate_dependency_name(name: &str) -> Result<()> {
    let valid = !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        && name
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphanumeric());
    if !valid {
        bail!("allowed_deps entry `{name}` is not a crate name");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Manifest {
        Manifest {
            schema: MANIFEST_SCHEMA,
            title: "slugify".to_string(),
            summary: "Deterministic slug generation.".to_string(),
            architecture_blueprint: ArchitectureBlueprint {
                tree: vec![
                    "Cargo.toml".to_string(),
                    ".gitignore".to_string(),
                    "src/lib.rs".to_string(),
                    "tests/integration.rs".to_string(),
                ],
                data_flow: "input -> normalize -> join".to_string(),
            },
            shared_contracts: vec![SharedContract {
                path: "src/lib.rs".to_string(),
                symbol: "slugify".to_string(),
                signature: "pub fn slugify(input: &str) -> String".to_string(),
            }],
            forger_tasks: vec![ForgerTask {
                id: "f1".to_string(),
                files: vec!["src/lib.rs".to_string()],
                contract_refs: vec!["slugify".to_string()],
                spec: "crate root".to_string(),
                verify: vec!["cargo build --quiet".to_string()],
            }],
            destroyer_tasks: vec![DestroyerTask {
                id: "d1".to_string(),
                targets: vec!["slugify".to_string()],
                kind: DestroyerKind::Integration,
                spec: "prove the contract end to end".to_string(),
                files: Vec::new(),
            }],
            allowed_deps: Vec::new(),
        }
    }

    #[test]
    fn accepts_a_total_blueprint() {
        base().validate().expect("blueprint should validate");
    }

    #[test]
    fn rejects_an_unowned_tree_path() {
        let mut manifest = base();
        manifest
            .architecture_blueprint
            .tree
            .push("src/orphan.rs".to_string());
        let error = manifest.validate().expect_err("orphan must be rejected");
        assert!(error.to_string().contains("src/orphan.rs"));
    }

    #[test]
    fn rejects_an_unknown_contract_reference() {
        let mut manifest = base();
        manifest.forger_tasks[0]
            .contract_refs
            .push("missing".to_string());
        let error = manifest.validate().expect_err("unknown contract must fail");
        assert!(error.to_string().contains("missing"));
    }

    #[test]
    fn rejects_path_traversal_and_absolute_paths() {
        let mut manifest = base();
        manifest.forger_tasks[0].files = vec!["../escape.rs".to_string()];
        assert!(manifest.validate().is_err());

        let mut manifest = base();
        manifest.forger_tasks[0].files = vec!["/etc/passwd".to_string()];
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn rejects_a_hallucinated_dependency_name() {
        let mut manifest = base();
        manifest.allowed_deps = vec!["serde = \"1\"".to_string()];
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn derives_a_crate_safe_package_name() {
        let manifest = base();
        assert_eq!(manifest.package_name(), "slugify");

        let mut manifest = base();
        manifest.title = "METAFUGE: Forge Engine!".to_string();
        assert_eq!(manifest.package_name(), "metafuge-forge-engine");
    }

    #[test]
    fn resolves_file_owners() {
        let manifest = base();
        assert_eq!(manifest.owner_of("src/lib.rs"), Some(("f1", "forger")));
        assert_eq!(
            manifest.owner_of("tests/integration.rs"),
            Some(("d1", "destroyer"))
        );
        assert_eq!(manifest.owner_of("Cargo.toml"), None);
    }
}
