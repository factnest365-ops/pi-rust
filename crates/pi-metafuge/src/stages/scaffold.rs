use std::fmt::Write;

use anyhow::{Context, Result};

use crate::manifest::Manifest;
use crate::run::{Run, Stage};
use crate::stages::StageOutcome;
use crate::support;

/// Scaffold stage: lay down the target, the package manifest, and the ignore
/// rules. Nothing here invents architecture; it renders what the blueprint says.
pub async fn run(run: &mut Run) -> Result<StageOutcome> {
    let manifest = run.manifest()?.clone();
    manifest
        .validate()
        .context("scaffold refused an invalid manifest")?;
    let out = run.state.out_dir.clone();

    std::fs::create_dir_all(run.telemetry_dir())
        .with_context(|| format!("could not create `{}`", run.telemetry_dir().display()))?;
    for path in &manifest.architecture_blueprint.tree {
        let relative = support::safe_relative(path)?;
        let Some(parent) = relative.parent() else {
            continue;
        };
        if parent.as_os_str().is_empty() {
            continue;
        }
        let directory = out.join(parent);
        std::fs::create_dir_all(&directory)
            .with_context(|| format!("could not create `{}`", directory.display()))?;
    }

    support::write_atomic(&out.join("Cargo.toml"), &render_cargo_toml(&manifest)?)?;
    support::write_atomic(&out.join(".gitignore"), "/target\n.metafuge/\n")?;
    support::write_atomic(
        &run.metafuge_dir().join("manifest.json"),
        &serde_json::to_string_pretty(&manifest)?,
    )?;
    run.save()?;
    run.report("scaffold written: Cargo.toml, blueprint directories, manifest mirror");
    Ok(StageOutcome::Advance(Stage::Forge))
}

/// Render the generated crate manifest. `allowed_deps` is the only source of
/// dependency lines; nothing is inherited from the forge host.
pub fn render_cargo_toml(manifest: &Manifest) -> Result<String> {
    let mut cargo = String::new();
    writeln!(cargo, "[package]")?;
    writeln!(cargo, "name = \"{}\"", manifest.package_name())?;
    writeln!(cargo, "version = \"0.1.0\"")?;
    writeln!(cargo, "edition = \"2021\"")?;
    writeln!(
        cargo,
        "description = \"{}\"",
        manifest.summary.replace('"', "'")
    )?;
    writeln!(cargo, "license = \"MIT\"")?;
    writeln!(cargo)?;
    writeln!(cargo, "[dependencies]")?;
    for dependency in &manifest.allowed_deps {
        writeln!(cargo, "{dependency} = \"*\"")?;
    }
    Ok(cargo)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deps;

    #[tokio::test]
    async fn writes_the_scaffold_from_the_blueprint() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut run = crate::testsupport::run(dir.path());
        crate::testsupport::advance_through(&mut run, Stage::Scaffold).await;

        let outcome = super::run(&mut run).await.expect("scaffold stage");
        assert_eq!(outcome, StageOutcome::Advance(Stage::Forge));

        let cargo = std::fs::read_to_string(dir.path().join("Cargo.toml")).expect("cargo");
        assert_eq!(deps::package_name(&cargo).as_deref(), Some("slugify"));
        assert!(cargo.contains("[dependencies]"));

        let ignore = std::fs::read_to_string(dir.path().join(".gitignore")).expect("ignore");
        assert!(ignore.contains("/target"));
        assert!(ignore.contains(".metafuge/"));

        assert!(dir.path().join("src").is_dir());
        assert!(dir.path().join("tests").is_dir());
        assert!(dir.path().join(".metafuge/telemetry").is_dir());
        assert!(dir.path().join(".metafuge/manifest.json").exists());
    }

    #[test]
    fn renders_only_authorized_dependencies() {
        let mut manifest = crate::testsupport::manifest();
        manifest.allowed_deps = vec!["serde".to_string()];
        let cargo = render_cargo_toml(&manifest).expect("render");
        let keys = deps::dependency_keys(&cargo);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].name, "serde");
    }
}
