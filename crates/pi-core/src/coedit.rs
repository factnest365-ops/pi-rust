use anyhow::{Result, anyhow};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;

/// Co-edit cluster: a set of files that frequently change together across commit history.
///
/// `support` is the number of commits in which this cluster's files were observed together
/// during pairwise expansion from the mined history.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CoEditCluster {
    pub files: Vec<String>,
    pub support: usize,
}

/// Computes co-edit clusters from git history for the repository at `repo_path`.
///
/// # Algorithm
/// 1. Run `git log --name-only --pretty=format:%H -n <max_commits>` to collect changed files per commit.
/// 2. Skip binary/vendor/noise paths: `target/`, `node_modules/`, and lockfiles (`*.lock`).
/// 3. Build pairwise co-occurrence counts across commits.
/// 4. Grow clusters by connecting pairs whose support meets `min_support`.
/// 5. Cap emitted clusters at `max_cluster_size` files.
/// 6. Sort deterministically by descending support, then lexicographic file list.
///
/// # Threshold semantics
/// - `min_support`: minimum commit count for a pairwise edge to be retained. Higher values yield
///   fewer, stronger clusters.
/// - `max_cluster_size`: hard cap on cluster cardinality to avoid runaway grouping.
/// - `max_commits`: history depth to inspect; older commits are ignored.
///
/// Returns `Ok(vec![])` if the directory is not a git repository or has no analyzable commits.
pub fn compute_coedit_clusters(
    repo_path: &Path,
    max_commits: usize,
    min_support: usize,
    max_cluster_size: usize,
) -> Result<Vec<CoEditCluster>> {
    if !is_git_repo(repo_path) {
        return Ok(vec![]);
    }

    let commits = parse_git_log(repo_path, max_commits)?;
    let filtered: Vec<HashSet<String>> = commits
        .into_iter()
        .filter_map(|files| {
            let mut set = HashSet::new();
            for f in files {
                if !is_noise_path(&f) {
                    set.insert(normalize_path(&f));
                }
            }
            if set.is_empty() { None } else { Some(set) }
        })
        .collect();

    if filtered.is_empty() {
        return Ok(vec![]);
    }

    // Pairwise co-occurrence counts.
    let mut pair_counts: HashMap<(String, String), usize> = HashMap::new();
    for files in &filtered {
        let mut sorted: Vec<String> = files.iter().cloned().collect();
        sorted.sort();
        for i in 0..sorted.len() {
            for j in (i + 1)..sorted.len() {
                let key = (sorted[i].clone(), sorted[j].clone());
                *pair_counts.entry(key).or_default() += 1;
            }
        }
    }

    // Build adjacency from pairs meeting min_support.
    let mut adjacency: HashMap<String, HashSet<String>> = HashMap::new();
    for ((a, b), count) in &pair_counts {
        if *count >= min_support {
            adjacency.entry(a.clone()).or_default().insert(b.clone());
            adjacency.entry(b.clone()).or_default().insert(a.clone());
        }
    }

    // Connected components as clusters, capped by max_cluster_size.
    let mut clusters: Vec<CoEditCluster> = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();
    for node in adjacency.keys().cloned().collect::<Vec<_>>() {
        if visited.contains(&node) {
            continue;
        }

        let mut component: Vec<String> = Vec::new();
        let mut stack = vec![node.clone()];
        while let Some(current) = stack.pop() {
            if visited.contains(&current) {
                continue;
            }
            visited.insert(current.clone());
            component.push(current.clone());
            if let Some(neighbors) = adjacency.get(&current) {
                for neighbor in neighbors {
                    if !visited.contains(neighbor) {
                        stack.push(neighbor.clone());
                    }
                }
            }
        }

        if component.len() > max_cluster_size {
            component.truncate(max_cluster_size);
        }

        // Support = sum of pairwise counts within the emitted cluster.
        let mut support = 0usize;
        component.sort();
        for i in 0..component.len() {
            for j in (i + 1)..component.len() {
                support += pair_counts
                    .get(&(component[i].clone(), component[j].clone()))
                    .copied()
                    .unwrap_or(0);
            }
        }

        clusters.push(CoEditCluster {
            files: component,
            support,
        });
    }

    // Sort deterministically: descending support, then lexicographic files.
    clusters.sort_by(|a, b| {
        b.support
            .cmp(&a.support)
            .then_with(|| a.files.cmp(&b.files))
    });

    Ok(clusters)
}

/// Returns files most frequently co-edited with `changed_file` in `repo_path`.
///
/// The search considers up to `max_commits` of history and only includes files with
/// co-occurrence count >= `min_support`. Results are sorted by descending co-occurrence,
/// then path lexicographically for determinism.
pub fn related_files(
    repo_path: &Path,
    changed_file: &str,
    max_commits: usize,
    min_support: usize,
) -> Vec<String> {
    let normalized = normalize_path(changed_file);
    let commits = match parse_git_log(repo_path, max_commits) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let mut counts: HashMap<String, usize> = HashMap::new();
    for files in commits {
        let mut set = HashSet::new();
        for f in files {
            let n = normalize_path(&f);
            if !is_noise_path(&n) {
                set.insert(n);
            }
        }
        if !set.contains(&normalized) {
            continue;
        }
        for other in set {
            if other != normalized {
                *counts.entry(other).or_default() += 1;
            }
        }
    }

    let mut related: Vec<(String, usize)> = counts
        .into_iter()
        .filter(|(_, c)| *c >= min_support)
        .collect();
    related.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    related.into_iter().map(|(path, _)| path).collect()
}

fn is_git_repo(repo_path: &Path) -> bool {
    let git_dir = repo_path.join(".git");
    if git_dir.exists() {
        return true;
    }

    let status = Command::new("git")
        .args([
            "-C",
            repo_path.to_str().unwrap_or("."),
            "rev-parse",
            "--git-dir",
        ])
        .output();

    status.map(|s| s.status.success()).unwrap_or(false)
}

fn parse_git_log(repo_path: &Path, max_commits: usize) -> Result<Vec<Vec<String>>> {
    let output = Command::new("git")
        .args([
            "-C",
            repo_path.to_str().unwrap_or("."),
            "log",
            "--name-only",
            "--pretty=format:%H",
            &format!("-n{}", max_commits),
        ])
        .output()?;

    if !output.status.success() {
        return Err(anyhow!(
            "git log failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut commits: Vec<Vec<String>> = Vec::new();

    // `git log --name-only --pretty=format:%H` separates commits with a single
    // blank line but emits NO separator after the final commit. Per git's
    // output contract, a commit hash is exactly the first line of the stream
    // or the line immediately following a blank separator, so use position,
    // not shape, to split commits. This stays correct even when a tracked
    // file itself is named like a 40-char hex string.
    let mut prev_blank = true;
    let mut current: Option<Vec<String>> = None;

    for line in stdout.lines() {
        let starts_commit = prev_blank;
        prev_blank = line.is_empty();
        if starts_commit {
            if let Some(done) = current.take().filter(|c| !c.is_empty()) {
                commits.push(done);
            }
            current = Some(Vec::new());
        } else if !line.is_empty() {
            current.get_or_insert_with(Vec::new).push(line.to_string());
        }
    }
    if let Some(done) = current.take().filter(|c| !c.is_empty()) {
        commits.push(done);
    }

    Ok(commits)
}

fn is_noise_path(path: &str) -> bool {
    if path.ends_with(".lock") {
        return true;
    }
    let components: Vec<&str> = path.split(['/', '\\']).collect();
    components
        .iter()
        .any(|c| *c == "target" || *c == "node_modules" || *c == ".git")
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_related_files_graceful_non_git() {
        let tmp = tempfile::tempdir().unwrap();
        let clusters = compute_coedit_clusters(tmp.path(), 100, 2, 10).unwrap();
        assert!(clusters.is_empty());

        let related = related_files(tmp.path(), "src/main.rs", 100, 1);
        assert!(related.is_empty());
    }

    #[test]
    fn test_compute_coedit_clusters_basic() {
        let repo = tempfile::tempdir().unwrap();
        init_git_repo(repo.path());

        // Commits 1 and 2 both touch {a, b}: pair support(a,b) = 2 >= min_support.
        // Commit 3 touches {c} alone: c joins no cluster.
        for n in 0..2 {
            fs::write(repo.path().join("a.txt"), format!("a{n}\n")).unwrap();
            fs::write(repo.path().join("b.txt"), format!("b{n}\n")).unwrap();
            git_add_commit(
                repo.path(),
                &format!("c{n}"),
                &["a.txt".to_string(), "b.txt".to_string()],
            );
        }

        fs::write(repo.path().join("c.txt"), "solo\n").unwrap();
        git_add_commit(repo.path(), "solo", &["c.txt".to_string()]);

        let clusters = compute_coedit_clusters(repo.path(), 100, 2, 10).unwrap();
        assert!(!clusters.is_empty());
        assert_eq!(
            clusters[0].files,
            vec!["a.txt".to_string(), "b.txt".to_string()]
        );
        assert_eq!(clusters[0].support, 2);
    }

    #[test]
    fn test_determinism() {
        let repo = tempfile::tempdir().unwrap();
        init_git_repo(repo.path());

        for i in 0..5 {
            fs::write(repo.path().join(format!("f{}.txt", i)), format!("{}\n", i)).unwrap();
            fs::write(repo.path().join(format!("g{}.txt", i)), format!("{}\n", i)).unwrap();
            git_add_commit(
                repo.path(),
                &format!("chore: {}", i),
                &[format!("f{}.txt", i), format!("g{}.txt", i)],
            );
        }

        let a = compute_coedit_clusters(repo.path(), 100, 2, 10).unwrap();
        let b = compute_coedit_clusters(repo.path(), 100, 2, 10).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn test_related_files_basic() {
        let repo = tempfile::tempdir().unwrap();
        init_git_repo(repo.path());

        fs::write(repo.path().join("a.rs"), "1\n").unwrap();
        fs::write(repo.path().join("b.rs"), "1\n").unwrap();
        git_add_commit(repo.path(), "x", &["a.rs".to_string(), "b.rs".to_string()]);

        fs::write(repo.path().join("a.rs"), "2\n").unwrap();
        fs::write(repo.path().join("b.rs"), "2\n").unwrap();
        git_add_commit(repo.path(), "y", &["a.rs".to_string(), "b.rs".to_string()]);

        let related = related_files(repo.path(), "a.rs", 100, 2);
        assert_eq!(related, vec!["b.rs".to_string()]);
    }

    #[test]
    fn test_noise_paths_skipped() {
        let repo = tempfile::tempdir().unwrap();
        init_git_repo(repo.path());

        let target_dir = repo.path().join("target").join("debug");
        fs::create_dir_all(&target_dir).unwrap();
        fs::write(target_dir.join("app"), "binary\n").unwrap();

        fs::write(repo.path().join("src.rs"), "1\n").unwrap();
        fs::write(repo.path().join("Cargo.lock"), "lock\n").unwrap();
        git_add_commit(
            repo.path(),
            "chore",
            &[
                "src.rs".to_string(),
                "Cargo.lock".to_string(),
                "target/debug/app".to_string(),
            ],
        );

        let clusters = compute_coedit_clusters(repo.path(), 100, 1, 10).unwrap();
        for c in clusters {
            assert!(
                c.files
                    .iter()
                    .all(|f| !f.contains("target") && !f.ends_with(".lock"))
            );
        }
    }

    fn init_git_repo(repo_path: &Path) {
        let _ = Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(repo_path)
            .output();

        let _ = Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo_path)
            .output();

        let _ = Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo_path)
            .output();
    }

    fn git_add_commit(repo_path: &Path, message: &str, files: &[String]) {
        for file in files {
            let _ = Command::new("git")
                .args(["add", file])
                .current_dir(repo_path)
                .output();
        }

        let _ = Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(repo_path)
            .output();
    }
}
