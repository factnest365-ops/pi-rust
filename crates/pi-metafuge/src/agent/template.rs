use anyhow::{Result, bail};
use async_trait::async_trait;

use crate::agent::{
    AgentDescription, AgentKind, DestroyRequest, DestroyResponse, ForgeRequest, ForgeResponse,
    GeneratedFile, JudgeDecision, JudgeRequest, JudgeResponse, MetafugeAgent, SpecRequest,
    SpecResponse,
};
use crate::manifest::{
    ArchitectureBlueprint, DestroyerKind, DestroyerTask, ForgerTask, MANIFEST_SCHEMA, Manifest,
    SharedContract,
};
use crate::verdict::JudgeSource;

/// Deterministic, offline, model-free agent.
///
/// It emits a real `slugify` crate so the whole loop can be proven in CI. Its
/// output is a fixture: no model wrote it, and nothing here pretends otherwise.
#[derive(Debug, Default, Clone, Copy)]
pub struct TemplateAgent;

pub const TEMPLATE_NOTE: &str =
    "deterministic offline template agent: emits a fixed slugify fixture, no model wrote this code";
const TEMPLATE_JUDGE_NOTE: &str =
    "template agent: mechanical gate cleared; this ship has no structural model judgment";

#[async_trait]
impl MetafugeAgent for TemplateAgent {
    fn describe(&self) -> AgentDescription {
        AgentDescription {
            kind: AgentKind::Template,
            name: "template".to_string(),
            note: TEMPLATE_NOTE.to_string(),
            model_backed: false,
        }
    }

    async fn spec(&self, _request: &SpecRequest) -> Result<SpecResponse> {
        Ok(SpecResponse {
            manifest: fixture_manifest(),
        })
    }

    async fn forge(&self, request: &ForgeRequest) -> Result<ForgeResponse> {
        let mut files = Vec::new();
        for path in &request.task.files {
            let contents = match path.as_str() {
                "src/lib.rs" => LIB_RS,
                "src/slug.rs" => SLUG_RS,
                other => bail!("template agent has no fixture for `{other}`"),
            };
            files.push(GeneratedFile {
                path: path.clone(),
                contents: contents.to_string(),
            });
        }
        Ok(ForgeResponse {
            files,
            notes: vec!["template fixture emitted; no model involved".to_string()],
        })
    }

    async fn destroy(&self, request: &DestroyRequest) -> Result<DestroyResponse> {
        let contents = match request.task.kind {
            DestroyerKind::Integration => TEST_INTEGRATION_RS,
            DestroyerKind::Boundary => TEST_BOUNDARY_RS,
            DestroyerKind::Fuzz => TEST_FUZZ_RS,
        };
        let files = request
            .task
            .target_files()
            .into_iter()
            .map(|path| GeneratedFile {
                path,
                contents: contents.to_string(),
            })
            .collect();
        Ok(DestroyResponse {
            files,
            notes: vec!["adversarial tests derived from contracts only".to_string()],
        })
    }

    async fn judge(&self, _request: &JudgeRequest) -> Result<JudgeResponse> {
        Ok(JudgeResponse {
            decision: JudgeDecision::Approve,
            violations: Vec::new(),
            rationale: format!("{TEMPLATE_JUDGE_NOTE} ({})", JudgeSource::Agent.as_str()),
        })
    }
}

pub(crate) fn fixture_manifest() -> Manifest {
    Manifest {
        schema: MANIFEST_SCHEMA,
        title: "slugify".to_string(),
        summary: "Deterministic slug generation over arbitrary text.".to_string(),
        architecture_blueprint: ArchitectureBlueprint {
            tree: [
                "Cargo.toml",
                ".gitignore",
                "src/lib.rs",
                "src/slug.rs",
                "tests/integration.rs",
                "tests/boundary.rs",
                "tests/fuzz.rs",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
            data_flow: "prompt -> blueprint + contracts -> forgers write src, destroyers write tests -> crucible -> director".to_string(),
        },
        shared_contracts: vec![SharedContract {
            path: "src/lib.rs".to_string(),
            symbol: "slugify".to_string(),
            signature: "pub fn slugify(input: &str) -> String".to_string(),
        }],
        forger_tasks: vec![
            ForgerTask {
                id: "f1".to_string(),
                files: vec!["src/lib.rs".to_string()],
                contract_refs: vec!["slugify".to_string()],
                spec: "Crate root: mount the slug module and re-export the contract symbol. Wiring only, no algorithm.".to_string(),
                verify: vec!["cargo build --quiet".to_string()],
            },
            ForgerTask {
                id: "f2".to_string(),
                files: vec!["src/slug.rs".to_string()],
                contract_refs: vec!["slugify".to_string()],
                spec: "Core algorithm: lowercase alphanumeric runs joined by single dashes, separators collapse, no leading or trailing dash, empty input yields an empty string.".to_string(),
                verify: vec!["cargo test --quiet".to_string()],
            },
        ],
        destroyer_tasks: vec![
            DestroyerTask {
                id: "d1".to_string(),
                targets: vec!["slugify".to_string()],
                kind: DestroyerKind::Integration,
                spec: "Representative sentences, punctuation, whitespace runs and idempotence against the published signature.".to_string(),
                files: vec!["tests/integration.rs".to_string()],
            },
            DestroyerTask {
                id: "d2".to_string(),
                targets: vec!["slugify".to_string()],
                kind: DestroyerKind::Boundary,
                spec: "Empty input, separator-only input, single characters, long input and unicode letters at the edges.".to_string(),
                files: vec!["tests/boundary.rs".to_string()],
            },
            DestroyerTask {
                id: "d3".to_string(),
                targets: vec!["slugify".to_string()],
                kind: DestroyerKind::Fuzz,
                spec: "Deterministic pseudo-random inputs must always yield well-formed slugs that are stable under re-slugging.".to_string(),
                files: vec!["tests/fuzz.rs".to_string()],
            },
        ],
        allowed_deps: Vec::new(),
    }
}

const LIB_RS: &str = r##"//! Deterministic slug generation.

mod slug;

pub use slug::slugify;
"##;

const SLUG_RS: &str = r##"/// Turn arbitrary text into a lowercase slug.
///
/// Unicode letters and digits survive; every other character is a separator.
/// Separator runs collapse into one `-`, and leading or trailing separators are
/// removed. Text without alphanumeric characters yields an empty string.
pub fn slugify(input: &str) -> String {
    let mut slug = String::with_capacity(input.len());
    let mut separator = false;
    for ch in input.chars() {
        if ch.is_alphanumeric() {
            if separator && !slug.is_empty() {
                slug.push('-');
            }
            separator = false;
            slug.extend(ch.to_lowercase().filter(|lower| lower.is_alphanumeric()));
        } else {
            separator = true;
        }
    }
    slug
}
"##;

const TEST_INTEGRATION_RS: &str = r##"use slugify::slugify;

#[test]
fn converts_sentences_into_slugs() {
    assert_eq!(slugify("Hello, World!"), "hello-world");
    assert_eq!(slugify("The Quick Brown Fox"), "the-quick-brown-fox");
    assert_eq!(slugify("  multiple   spaces  "), "multiple-spaces");
}

#[test]
fn is_idempotent() {
    for input in ["Hello, World!", "METAFUGE  FORGE", "a-b_c"] {
        let once = slugify(input);
        assert_eq!(slugify(&once), once);
    }
}

#[test]
fn keeps_unicode_letters() {
    assert_eq!(slugify("Café Olé"), "café-olé");
}
"##;

const TEST_BOUNDARY_RS: &str = r##"use slugify::slugify;

#[test]
fn empty_input_yields_empty_slug() {
    assert_eq!(slugify(""), "");
}

#[test]
fn separator_only_input_yields_empty_slug() {
    assert_eq!(slugify("---___   ..."), "");
}

#[test]
fn single_alphanumeric_character_survives() {
    assert_eq!(slugify("A"), "a");
}

#[test]
fn long_input_collapses_instead_of_growing() {
    let input = "Metafuge  ".repeat(1_000);
    let slug = slugify(&input);
    assert_eq!(slug.matches("metafuge").count(), 1_000);
    assert!(!slug.contains("--"));
    assert!(slug.len() <= input.len());
    assert_eq!(slugify(&slug), slug);
}

#[test]
fn separators_never_lead_or_trail() {
    for input in ["! leading", "trailing !", "! both !", "\t\n newline \n"] {
        let slug = slugify(input);
        assert!(!slug.starts_with('-'));
        assert!(!slug.ends_with('-'));
    }
}
"##;

const TEST_FUZZ_RS: &str = r##"use slugify::slugify;

fn well_formed(text: &str) -> bool {
    text.chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
        && !text.starts_with('-')
        && !text.ends_with('-')
        && !text.contains("--")
}

#[test]
fn random_inputs_always_emit_well_formed_stable_slugs() {
    let alphabet: [char; 12] = [
        'a', 'B', 'z', '0', '9', ' ', '-', '_', '\t', '\n', '.', '!',
    ];
    let mut state: u64 = 0x5eed;
    for _ in 0..2_000 {
        let mut input = String::new();
        for _ in 0..32 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let index = (state >> 33) as usize % alphabet.len();
            input.push(alphabet[index]);
        }
        let slug = slugify(&input);
        assert!(well_formed(&slug), "input {input:?} produced {slug:?}");
        assert_eq!(slugify(&slug), slug);
    }
}
"##;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fixture_manifest_validates() {
        let response = TemplateAgent
            .spec(&SpecRequest {
                schema: MANIFEST_SCHEMA,
                prompt: "slugify things".to_string(),
                stack: "rust".to_string(),
            })
            .await
            .expect("spec");
        response.manifest.validate().expect("fixture is valid");
        assert_eq!(response.manifest.title, "slugify");
        assert!(response.manifest.allowed_deps.is_empty());
    }

    #[tokio::test]
    async fn forge_emits_the_contract_symbol() {
        let manifest = fixture_manifest();
        let task = manifest.forger_tasks[1].clone();
        let response = TemplateAgent
            .forge(&ForgeRequest {
                round: 1,
                task,
                contracts: manifest.shared_contracts.clone(),
                blueprint: manifest.architecture_blueprint.clone(),
                diagnostic: None,
            })
            .await
            .expect("forge");
        assert_eq!(response.files.len(), 1);
        assert!(response.files[0].contents.contains("pub fn slugify"));
    }

    #[tokio::test]
    async fn destroy_never_receives_forger_source() {
        let manifest = fixture_manifest();
        let task = manifest.destroyer_tasks[0].clone();
        let response = TemplateAgent
            .destroy(&DestroyRequest {
                round: 1,
                task,
                contracts: manifest.shared_contracts.clone(),
                blueprint: manifest.architecture_blueprint.clone(),
                diagnostic: None,
            })
            .await
            .expect("destroy");
        assert!(response.files[0].contents.contains("use slugify::slugify;"));
        assert!(!response.files[0].contents.contains("pub fn slugify"));
    }

    #[tokio::test]
    async fn describe_names_the_template_agent() {
        let description = TemplateAgent.describe();
        assert_eq!(description.kind, AgentKind::Template);
        assert!(!description.model_backed);
        assert!(description.note.contains("no model wrote this code"));
    }
}
