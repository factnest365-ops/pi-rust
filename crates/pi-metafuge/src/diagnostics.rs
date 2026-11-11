use std::collections::BTreeSet;

use crate::manifest::Manifest;

/// A diagnostic, not a log: error lines plus two lines of context, hard capped.
pub const MAX_DIAGNOSTIC_LINES: usize = 40;
pub const CONTEXT_LINES: usize = 2;

const ERROR_MARKERS: [&str; 7] = [
    "error[",
    "error:",
    "panicked at",
    "assertion",
    "FAILED",
    "fatal:",
    "could not compile",
];

/// Trim a compiler or test log down to the failure window.
///
/// The worker gets the error lines plus two lines of context. It never gets the
/// whole log, which is the difference between a targeted repair and a rerun.
pub fn trim(log: &str) -> String {
    let lines: Vec<&str> = log.lines().collect();
    if lines.is_empty() {
        return String::new();
    }
    let last = lines.len() - 1;
    let mut keep = BTreeSet::new();
    for (index, line) in lines.iter().enumerate() {
        if ERROR_MARKERS.iter().any(|marker| line.contains(marker)) {
            let start = index.saturating_sub(CONTEXT_LINES);
            let end = (index + CONTEXT_LINES).min(last);
            for picked in start..=end {
                keep.insert(picked);
            }
        }
    }
    if keep.is_empty() {
        let mut tail: Vec<&str> = lines
            .iter()
            .rev()
            .filter(|line| !line.trim().is_empty())
            .take(10)
            .copied()
            .collect();
        tail.reverse();
        return tail.join("\n");
    }
    let mut out: Vec<String> = Vec::new();
    let mut previous: Option<usize> = None;
    for index in keep.into_iter().take(MAX_DIAGNOSTIC_LINES) {
        if let Some(previous) = previous
            && index > previous + 1
        {
            out.push("...".to_string());
        }
        out.push(lines[index].to_string());
        previous = Some(index);
    }
    out.join("\n")
}

/// Pull Rust source paths out of compiler and test output.
pub fn extract_paths(log: &str) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    for line in log.lines() {
        let candidate = match line.find("-->") {
            Some(index) => &line[index + 3..],
            None => line,
        };
        for token in candidate.split_whitespace() {
            let token = token.trim_matches(|ch| ch == '`' || ch == '"' || ch == '\'');
            let path = token.split(':').next().unwrap_or(token);
            if path.ends_with(".rs") && !paths.iter().any(|seen| seen == path) {
                paths.push(path.to_string());
            }
        }
    }
    paths
}

/// Which tasks own the failure, so only they are re-issued.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RepairTargets {
    pub forgers: BTreeSet<String>,
    pub destroyers: BTreeSet<String>,
}

impl RepairTargets {
    pub fn is_empty(&self) -> bool {
        self.forgers.is_empty() && self.destroyers.is_empty()
    }

    pub fn ids(&self) -> Vec<String> {
        self.forgers
            .iter()
            .chain(self.destroyers.iter())
            .cloned()
            .collect()
    }
}

/// Map a failure to owners.
///
/// A compile error belongs to the file's owner. A failing assertion belongs to
/// the forgers that implement the destroyer's target contracts, because the test
/// is the spec and the code has to satisfy it. When nothing can be mapped the
/// loop re-issues every forger, which the round cap bounds.
pub fn map_failures(log: &str, manifest: &Manifest) -> RepairTargets {
    let compile_error = log.contains("error[") || log.contains("error:");
    let mut targets = RepairTargets::default();
    for path in extract_paths(log) {
        match manifest.owner_of(&path) {
            Some((id, "forger")) => {
                targets.forgers.insert(id.to_string());
            }
            Some((id, "destroyer")) => {
                if compile_error {
                    targets.destroyers.insert(id.to_string());
                }
                if let Some(task) = manifest.destroyer_task(id) {
                    for forger in &manifest.forger_tasks {
                        if forger
                            .contract_refs
                            .iter()
                            .any(|reference| task.targets.contains(reference))
                        {
                            targets.forgers.insert(forger.id.clone());
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if targets.is_empty() {
        for task in &manifest.forger_tasks {
            targets.forgers.insert(task.id.clone());
        }
    }
    targets
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testsupport::manifest;

    #[test]
    fn trim_keeps_the_error_and_two_lines_of_context() {
        let log = "Compiling slugify v0.1.0\n\
                   line one\n\
                   line two\n\
                   error[E0308]: mismatched types\n\
                   line four\n\
                   line five\n\
                   line six\n\
                   line seven";
        let trimmed = trim(log);
        assert!(trimmed.contains("error[E0308]"));
        assert!(trimmed.contains("line two"));
        assert!(trimmed.contains("line five"));
        assert!(!trimmed.contains("line seven"));
        assert!(!trimmed.contains("Compiling"));
    }

    #[test]
    fn trim_never_exceeds_the_cap() {
        let log = "error: boom\n".repeat(200);
        let trimmed = trim(&log);
        assert!(trimmed.lines().count() <= MAX_DIAGNOSTIC_LINES);
    }

    #[test]
    fn trim_falls_back_to_the_tail_when_nothing_matches() {
        let log = "alpha\nbeta\ngamma";
        assert_eq!(trim(log), "alpha\nbeta\ngamma");
    }

    #[test]
    fn extracts_compiler_locations() {
        let log = "error[E0308]: mismatched types\n  --> src/slug.rs:10:5\n   |\n10 | foo\n";
        assert_eq!(extract_paths(log), vec!["src/slug.rs".to_string()]);
    }

    #[test]
    fn extracts_test_locations_without_arrows() {
        let log = "thread 'main' panicked at tests/boundary.rs:12:5:\nassertion failed";
        assert_eq!(extract_paths(log), vec!["tests/boundary.rs".to_string()]);
    }

    #[test]
    fn a_forger_compile_error_repairs_only_that_forger() {
        let manifest = manifest();
        let log = "error[E0308]: mismatched types\n --> src/slug.rs:3:1";
        let targets = map_failures(log, &manifest);
        assert_eq!(targets.forgers, BTreeSet::from(["f2".to_string()]));
        assert!(targets.destroyers.is_empty());
    }

    #[test]
    fn a_failing_assertion_repairs_the_targeted_forgers() {
        let manifest = manifest();
        let log = "panicked at tests/boundary.rs:12:5:\nassertion `left == right` failed";
        let targets = map_failures(log, &manifest);
        assert_eq!(
            targets.forgers,
            BTreeSet::from(["f1".to_string(), "f2".to_string()])
        );
        assert!(targets.destroyers.is_empty());
    }

    #[test]
    fn a_test_compile_error_repairs_the_destroyer_and_its_forgers() {
        let manifest = manifest();
        let log = "error[E0425]: cannot find value\n --> tests/fuzz.rs:4:9";
        let targets = map_failures(log, &manifest);
        assert_eq!(targets.destroyers, BTreeSet::from(["d3".to_string()]));
        assert!(!targets.forgers.is_empty());
    }

    #[test]
    fn an_unmappable_failure_repairs_every_forger() {
        let manifest = manifest();
        let log = "error: linker exploded";
        let targets = map_failures(log, &manifest);
        assert_eq!(targets.forgers.len(), manifest.forger_tasks.len());
    }
}
