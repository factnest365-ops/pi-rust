use anyhow::Result;
use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader};

pub struct AstTool;

impl AstTool {
    pub fn execute(args: &Value) -> Result<String> {
        let file_path = args["path"]
            .as_str()
            .or_else(|| args["file"].as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'path' argument for ast tool"))?;

        if let Some(symbol) = args["symbol"].as_str().or_else(|| args["name"].as_str()) {
            Self::slice_symbol(file_path, symbol)
        } else {
            Self::outline_structure(file_path)
        }
    }

    /// Surgically extracts the full block of a symbol using brace/indent tracking
    pub fn slice_symbol(file_path: &str, symbol: &str) -> Result<String> {
        let file = File::open(file_path)?;
        let lines: Vec<String> = BufReader::new(file).lines().collect::<Result<_, _>>()?;

        let mut start_idx = None;

        for (idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if !trimmed.starts_with("//")
                && !trimmed.starts_with('#')
                && crate::lsp::LspTool::matches_exact_symbol(trimmed, symbol)
            {
                start_idx = Some(idx);
                break;
            }
        }

        let Some(start) = start_idx else {
            return Err(anyhow::anyhow!("Symbol '{}' not found in {}", symbol, file_path));
        };

        // Track opening/closing delimiters
        let mut brace_depth: i32 = 0;
        let mut end_idx = start;
        let mut saw_open_brace = false;

        let is_python = file_path.ends_with(".py");

        if is_python {
            let base_indent = lines[start].len() - lines[start].trim_start().len();
            for (idx, l) in lines.iter().enumerate().skip(start + 1) {
                if l.trim().is_empty() || l.trim().starts_with('#') {
                    continue;
                }
                let indent = l.len() - l.trim_start().len();
                if indent <= base_indent {
                    break;
                }
                end_idx = idx;
            }
        } else {
            for (idx, line) in lines.iter().enumerate().skip(start) {
                let (open_count, close_count, has_semicolon) = Self::count_braces(line);

                if open_count > 0 {
                    saw_open_brace = true;
                }

                brace_depth += open_count - close_count;
                end_idx = idx;

                if saw_open_brace && brace_depth <= 0 {
                    break;
                }

                if !saw_open_brace && has_semicolon {
                    break;
                }
            }
        }

        let mut output = format!(
            "--- AST Symbol Slice: '{}' in {}:{}-{} ---\n",
            symbol, file_path, start + 1, end_idx + 1
        );

        for idx in start..=end_idx {
            if idx < lines.len() {
                output.push_str(&format!("{:4} | {}\n", idx + 1, lines[idx]));
            }
        }

        Ok(output)
    }

    /// Outlines top-level definitions and structure of a file
    pub fn outline_structure(file_path: &str) -> Result<String> {
        let file = File::open(file_path)?;
        let lines: Vec<String> = BufReader::new(file).lines().collect::<Result<_, _>>()?;

        let mut outline = format!("--- File Structure: {} ({} lines) ---\n", file_path, lines.len());

        for (idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("pub fn ")
                || trimmed.starts_with("fn ")
                || trimmed.starts_with("async fn ")
                || trimmed.starts_with("pub async fn ")
                || trimmed.starts_with("pub(crate) fn ")
                || trimmed.starts_with("pub(crate) async fn ")
                || trimmed.starts_with("pub struct ")
                || trimmed.starts_with("pub(crate) struct ")
                || trimmed.starts_with("struct ")
                || trimmed.starts_with("pub enum ")
                || trimmed.starts_with("pub(crate) enum ")
                || trimmed.starts_with("enum ")
                || trimmed.starts_with("pub trait ")
                || trimmed.starts_with("pub(crate) trait ")
                || trimmed.starts_with("trait ")
                || trimmed.starts_with("impl ")
                || trimmed.starts_with("impl<")
                || trimmed.starts_with("class ")
                || trimmed.starts_with("def ")
                || trimmed.starts_with("async def ")
            {
                outline.push_str(&format!("  line {:4} | {}\n", idx + 1, trimmed));
            }
        }

        Ok(outline)
    }

    fn count_braces(line: &str) -> (i32, i32, bool) {
        let mut in_str = false;
        let mut is_escaped = false;
        let mut open = 0;
        let mut close = 0;
        let mut has_semicolon = false;

        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if in_str {
                if is_escaped {
                    is_escaped = false;
                } else if c == '\\' {
                    is_escaped = true;
                } else if c == '"' {
                    in_str = false;
                }
            } else if c == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
                break;
            } else if c == '\'' {
                // Skip character literals like '{', '}', '\'', '\\'
                if i + 2 < chars.len() && chars[i + 2] == '\'' {
                    i += 3;
                    continue;
                } else if i + 3 < chars.len() && chars[i + 1] == '\\' && chars[i + 3] == '\'' {
                    i += 4;
                    continue;
                }
            } else if c == '"' {
                in_str = true;
            } else if c == '{' {
                open += 1;
            } else if c == '}' {
                close += 1;
            } else if c == ';' {
                has_semicolon = true;
            }
            i += 1;
        }

        (open, close, has_semicolon)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_slice_symbol() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("calculator.rs");
        let file_str = file_path.to_str().unwrap();

        let code = r#"
pub struct Calculator;

impl Calculator {
    pub fn add(a: i32, b: i32) -> i32 {
        let sum = a + b;
        sum
    }

    pub fn sub(a: i32, b: i32) -> i32 {
        a - b
    }
}
"#;
        fs::write(&file_path, code).unwrap();

        let slice = AstTool::slice_symbol(file_str, "add").unwrap();
        assert!(slice.contains("pub fn add(a: i32, b: i32) -> i32"));
        assert!(slice.contains("let sum = a + b;"));
        assert!(slice.contains("sum"));
        assert!(!slice.contains("pub fn sub"));

        let outline = AstTool::outline_structure(file_str).unwrap();
        assert!(outline.contains("pub struct Calculator;"));
        assert!(outline.contains("pub fn add"));
        assert!(outline.contains("pub fn sub"));
    }

    #[test]
    fn test_slice_trait_and_braces_in_strings() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("traits.rs");
        let file_str = file_path.to_str().unwrap();

        let code = r#"
pub trait Runner {
    fn run(&self) -> bool;
    fn stop(&self) -> bool;
}

pub fn print_json() {
    let s = "{ not a real brace }";
    println!("{}", s);
}
"#;
        fs::write(&file_path, code).unwrap();

        let slice = AstTool::slice_symbol(file_str, "run").unwrap();
        assert!(slice.contains("fn run(&self) -> bool;"));
        assert!(!slice.contains("fn stop"));

        let slice_json = AstTool::slice_symbol(file_str, "print_json").unwrap();
        assert!(slice_json.contains("pub fn print_json()"));
        assert!(slice_json.contains("println!(\"{}\", s);"));
    }
}
