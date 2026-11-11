use serde::{Deserialize, Serialize};

use crate::verdict::Violation;

/// Hard ceiling for a generated source file. Bloat is treason.
pub const MAX_SOURCE_LINES: usize = 400;

/// One generated source file, as read back from disk for the gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceFile {
    pub path: String,
    pub contents: String,
}

/// Run the mechanical director rules.
///
/// These are deterministic text checks, not model judgment. They run before any
/// agent is asked to judge, and a violation here makes approval impossible.
pub fn inspect(files: &[SourceFile]) -> Vec<Violation> {
    let mut violations = Vec::new();
    for file in files {
        if !file.path.ends_with(".rs") {
            continue;
        }
        let lines: Vec<&str> = file.contents.lines().collect();
        if lines.len() > MAX_SOURCE_LINES {
            violations.push(Violation::new(
                &file.path,
                MAX_SOURCE_LINES + 1,
                "file-too-long",
                format!(
                    "file has {} lines, the ceiling is {MAX_SOURCE_LINES}",
                    lines.len()
                ),
            ));
        }
        // An integration test target is a test by definition: panics there are
        // assertions, not lazy error handling.
        if file.path.starts_with("tests/") {
            continue;
        }
        violations.extend(inspect_module(&file.path, &lines));
    }
    violations
}

fn inspect_module(path: &str, lines: &[&str]) -> Vec<Violation> {
    let mut violations = Vec::new();
    let mut depth: i32 = 0;
    let mut test_region: Option<i32> = None;
    let mut armed = false;
    for (index, raw) in lines.iter().enumerate() {
        let line_no = index + 1;
        let code = strip_line_comment(raw);
        let starts_test = code.contains("#[cfg(test)]");
        let begins_test_module =
            (armed || starts_test) && code.contains("mod ") && code.contains('{');
        let inside_test = test_region.is_some() || begins_test_module || starts_test;
        if !inside_test {
            if code.contains(".unwrap()") {
                violations.push(Violation::new(
                    path,
                    line_no,
                    "panic-outside-tests",
                    "`unwrap()` outside tests; handle the error or prove it impossible",
                ));
            }
            if code.contains(".expect(") {
                violations.push(Violation::new(
                    path,
                    line_no,
                    "panic-outside-tests",
                    "`expect()` outside tests; lazy error handling is rejected",
                ));
            }
            if code.contains("todo!(") || code.contains("unimplemented!(") {
                violations.push(Violation::new(
                    path,
                    line_no,
                    "dead-macro",
                    "`todo!()`/`unimplemented!()` on a shipped path; finish the code",
                ));
            }
        }
        for ch in code.chars() {
            match ch {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
        }
        if begins_test_module {
            test_region = Some(depth);
        }
        if let Some(region) = test_region
            && depth < region
        {
            test_region = None;
        }
        armed = starts_test;
    }
    violations
}

fn strip_line_comment(line: &str) -> &str {
    match line.find("//") {
        Some(index) => &line[..index],
        None => line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, contents: &str) -> SourceFile {
        SourceFile {
            path: path.to_string(),
            contents: contents.to_string(),
        }
    }

    #[test]
    fn a_clean_file_has_no_violations() {
        let files = vec![file(
            "src/lib.rs",
            "pub fn slugify(input: &str) -> String {\n    input.to_string()\n}\n",
        )];
        assert!(inspect(&files).is_empty());
    }

    #[test]
    fn unwrap_and_expect_outside_tests_are_rejected() {
        let files = vec![file(
            "src/lib.rs",
            "pub fn a() -> u8 {\n    let x: Option<u8> = None;\n    x.unwrap()\n}\n\
             pub fn b() -> u8 {\n    let x: Option<u8> = None;\n    x.expect(\"boom\")\n}\n",
        )];
        let violations = inspect(&files);
        assert_eq!(violations.len(), 2);
        assert_eq!(violations[0].line, 3);
        assert_eq!(violations[1].line, 7);
        assert!(violations.iter().all(|v| v.rule == "panic-outside-tests"));
    }

    #[test]
    fn unwrap_inside_a_cfg_test_module_is_allowed() {
        let files = vec![file(
            "src/lib.rs",
            "pub fn a() -> u8 { 1 }\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {\n        let x: Option<u8> = None;\n        assert!(x.unwrap() == 0);\n    }\n}\n",
        )];
        assert!(inspect(&files).is_empty());
    }

    #[test]
    fn unwrap_after_a_test_module_closes_is_rejected_again() {
        let files = vec![file(
            "src/lib.rs",
            "#[cfg(test)]\nmod tests {\n    fn t() { let x: Option<u8> = None; x.unwrap(); }\n}\n\
             pub fn a() -> u8 { let x: Option<u8> = None; x.unwrap() }\n",
        )];
        let violations = inspect(&files);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].line, 5);
    }

    #[test]
    fn integration_test_targets_may_panic() {
        let files = vec![file(
            "tests/integration.rs",
            "use slugify::slugify;\n#[test]\nfn t() { assert_eq!(slugify(\"a\"), \"a\".to_string()); }\n",
        )];
        assert!(inspect(&files).is_empty());
    }

    #[test]
    fn a_file_over_the_ceiling_is_rejected() {
        let contents = "// filler\n".repeat(MAX_SOURCE_LINES + 1);
        let violations = inspect(&[file("src/big.rs", &contents)]);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].rule, "file-too-long");
        assert_eq!(violations[0].line, MAX_SOURCE_LINES + 1);
    }

    #[test]
    fn todo_macros_are_rejected() {
        let files = vec![file(
            "src/lib.rs",
            "pub fn a() -> u8 {\n    todo!(\"later\")\n}\n",
        )];
        let violations = inspect(&files);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].rule, "dead-macro");
    }
}
