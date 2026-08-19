use anyhow::Result;
use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader};

pub struct AstTool;

#[derive(Debug, Clone)]
pub struct AstSymbolBlock {
    pub name: String,
    pub kind: String,
    pub signature: String,
    pub start_line: usize,
    pub end_line: usize,
}

impl AstTool {
    pub fn execute(args: &Value) -> Result<String> {
        let file_path = args["path"]
            .as_str()
            .or_else(|| args["file"].as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'path' argument for ast tool"))?;

        let action = args["action"]
            .as_str()
            .or_else(|| args["command"].as_str())
            .unwrap_or("");

        let is_dep_slice = action == "dependency_slice"
            || action == "dependencies"
            || action == "slice_dependencies"
            || args["dependencies"].as_bool().unwrap_or(false)
            || args["dep_slice"].as_bool().unwrap_or(false);

        if is_dep_slice {
            let symbol = args["symbol"]
                .as_str()
                .or_else(|| args["name"].as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing 'symbol' argument for dependency slice"))?;
            Self::extract_dependency_slice(file_path, symbol)
        } else if let Some(symbol) = args["symbol"].as_str().or_else(|| args["name"].as_str()) {
            Self::slice_symbol(file_path, symbol)
        } else {
            Self::outline_structure(file_path)
        }
    }

    /// Surgically extracts the full block of a symbol using brace/indent tracking
    pub fn slice_symbol(file_path: &str, symbol: &str) -> Result<String> {
        let file = File::open(file_path)?;
        let lines: Vec<String> = BufReader::new(file).lines().collect::<Result<_, _>>()?;

        let Some((start, end)) = Self::find_symbol_range(&lines, file_path, symbol) else {
            return Err(anyhow::anyhow!("Symbol '{}' not found in {}", symbol, file_path));
        };

        let mut output = format!(
            "--- AST Symbol Slice: '{}' in {}:{}-{} ---\n",
            symbol,
            file_path,
            start + 1,
            end + 1
        );

        for idx in start..=end {
            if idx < lines.len() {
                output.push_str(&format!("{:4} | {}\n", idx + 1, lines[idx]));
            }
        }

        Ok(output)
    }

    /// Extracts a high-signal dependency context slice for a symbol:
    /// parses the target symbol definition, struct fields, traits, parent context,
    /// caller signatures, and callee signatures to cut context token waste.
    pub fn extract_dependency_slice(file_path: &str, symbol_name: &str) -> Result<String> {
        let file = File::open(file_path)?;
        let lines: Vec<String> = BufReader::new(file).lines().collect::<Result<_, _>>()?;

        let Some((target_start, target_end)) = Self::find_symbol_range(&lines, file_path, symbol_name) else {
            return Err(anyhow::anyhow!("Symbol '{}' not found in {}", symbol_name, file_path));
        };

        let all_symbols = Self::discover_all_symbols(&lines, file_path);
        let target_body = lines[target_start..=target_end].join("\n");

        let mut callees: Vec<&AstSymbolBlock> = Vec::new();
        let mut callers: Vec<&AstSymbolBlock> = Vec::new();
        let mut related_types: Vec<&AstSymbolBlock> = Vec::new();

        // 1. Discover Callees (functions/methods invoked by target)
        for sym in &all_symbols {
            if sym.start_line == target_start && sym.end_line == target_end {
                continue;
            }
            if (sym.kind == "Function" || sym.kind == "Method")
                && Self::contains_identifier(&target_body, &sym.name)
                && !callees.iter().any(|c| c.name == sym.name)
            {
                callees.push(sym);
            }
        }

        // 2. Discover Callers (functions/methods calling target)
        for sym in &all_symbols {
            if sym.start_line == target_start && sym.end_line == target_end {
                continue;
            }
            if sym.kind == "Function" || sym.kind == "Method" {
                let caller_body = lines[sym.start_line..=sym.end_line].join("\n");
                if Self::contains_identifier(&caller_body, symbol_name)
                    && !callers.iter().any(|c| c.name == sym.name)
                {
                    callers.push(sym);
                }
            }
        }

        // 3. Discover Enclosing Impl / Class and Related Types
        for sym in &all_symbols {
            if sym.start_line == target_start && sym.end_line == target_end {
                continue;
            }
            if sym.kind == "Implementation" || sym.kind == "Class" {
                // If this impl/class encloses target, or references target
                if (sym.start_line <= target_start && sym.end_line >= target_end)
                    || Self::contains_identifier(&sym.signature, symbol_name)
                {
                    if !related_types.iter().any(|t| t.start_line == sym.start_line) {
                        related_types.push(sym);
                    }
                    // Also find the struct/trait corresponding to sym.name
                    if !sym.name.is_empty() {
                        for t in &all_symbols {
                            if (t.kind == "Struct" || t.kind == "Enum" || t.kind == "Trait" || t.kind == "Class")
                                && t.name == sym.name
                                && !related_types.iter().any(|r| r.name == t.name && r.kind == t.kind)
                            {
                                related_types.push(t);
                            }
                        }
                    }
                }
            } else if (sym.kind == "Struct" || sym.kind == "Enum" || sym.kind == "Trait")
                && (Self::contains_identifier(&target_body, &sym.name)
                    || target_body.contains(&format!("&{}", sym.name))
                    || target_body.contains(&format!(": {}", sym.name)))
                && !related_types.iter().any(|t| t.name == sym.name && t.kind == sym.kind)
            {
                related_types.push(sym);
            }
        }

        // 4. Extract Top-Level Imports / Use Statements
        let mut imports = Vec::new();
        for (idx, line) in lines.iter().enumerate().take(60) {
            let trimmed = line.trim();
            if trimmed.starts_with("use ")
                || trimmed.starts_with("pub use ")
                || trimmed.starts_with("import ")
                || trimmed.starts_with("from ")
            {
                imports.push(format!("{:4} | {}", idx + 1, trimmed));
            }
        }

        // 5. Build Formatted High-Signal AST Slice
        let mut out = String::new();
        out.push_str("================================================================================\n");
        out.push_str(&format!(
            "AST DEPENDENCY CONTEXT SLICE: '{}'\nFile: {} (Lines {}-{})\n",
            symbol_name,
            file_path,
            target_start + 1,
            target_end + 1
        ));
        out.push_str("================================================================================\n\n");

        out.push_str("[Target Definition]\n");
        for idx in target_start..=target_end {
            if idx < lines.len() {
                out.push_str(&format!("{:4} | {}\n", idx + 1, lines[idx]));
            }
        }
        out.push('\n');

        if !related_types.is_empty() {
            out.push_str("[Associated Types, Structs & Traits]\n");
            for sym in &related_types {
                out.push_str(&format!("  line {:4} | [{}] {}\n", sym.start_line + 1, sym.kind, sym.signature));
                if sym.end_line > sym.start_line && sym.end_line - sym.start_line <= 12 {
                    for i in (sym.start_line + 1)..=sym.end_line {
                        if i < lines.len() {
                            out.push_str(&format!("         | {}\n", lines[i]));
                        }
                    }
                }
            }
            out.push('\n');
        }

        out.push_str(&format!("[Callee Signatures (Invoked by '{}')]\n", symbol_name));
        if callees.is_empty() {
            out.push_str("  (None detected in file scope)\n");
        } else {
            for c in callees {
                out.push_str(&format!("  line {:4} | {}\n", c.start_line + 1, c.signature));
            }
        }
        out.push('\n');

        out.push_str(&format!("[Caller Signatures (Calls '{}')]\n", symbol_name));
        if callers.is_empty() {
            out.push_str("  (None detected in file scope)\n");
        } else {
            for c in callers {
                out.push_str(&format!("  line {:4} | {}\n", c.start_line + 1, c.signature));
            }
        }
        out.push('\n');

        if !imports.is_empty() {
            out.push_str("[Imports & Scope Context]\n");
            for imp in imports {
                out.push_str(&format!("{}\n", imp));
            }
            out.push('\n');
        }

        out.push_str("================================================================================\n");

        Ok(out)
    }

    /// Finds the `(start_idx, end_idx)` line range of a symbol in a file
    pub fn find_symbol_range(
        lines: &[String],
        file_path: &str,
        symbol: &str,
    ) -> Option<(usize, usize)> {
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

        let start = start_idx?;
        let is_python = file_path.ends_with(".py");
        let mut end_idx = start;

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
            let mut brace_depth: i32 = 0;
            let mut saw_open_brace = false;

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

        Some((start, end_idx))
    }

    /// Scans the file lines and discovers all function, struct, enum, trait, class, and impl blocks
    pub fn discover_all_symbols(lines: &[String], file_path: &str) -> Vec<AstSymbolBlock> {
        let is_python = file_path.ends_with(".py");
        let mut symbols = Vec::new();

        for (idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with('#') || trimmed.starts_with("/*") {
                continue;
            }

            let (name, kind) = if trimmed.starts_with("pub fn ")
                || trimmed.starts_with("fn ")
                || trimmed.starts_with("async fn ")
                || trimmed.starts_with("pub async fn ")
                || trimmed.starts_with("pub(crate) fn ")
                || trimmed.starts_with("pub(crate) async fn ")
            {
                (Self::extract_symbol_name(trimmed, "fn "), "Function".to_string())
            } else if trimmed.starts_with("pub struct ")
                || trimmed.starts_with("pub(crate) struct ")
                || trimmed.starts_with("struct ")
            {
                (Self::extract_symbol_name(trimmed, "struct "), "Struct".to_string())
            } else if trimmed.starts_with("pub enum ")
                || trimmed.starts_with("pub(crate) enum ")
                || trimmed.starts_with("enum ")
            {
                (Self::extract_symbol_name(trimmed, "enum "), "Enum".to_string())
            } else if trimmed.starts_with("pub trait ")
                || trimmed.starts_with("pub(crate) trait ")
                || trimmed.starts_with("trait ")
            {
                (Self::extract_symbol_name(trimmed, "trait "), "Trait".to_string())
            } else if trimmed.starts_with("impl ") || trimmed.starts_with("impl<") {
                let name = Self::extract_impl_target(trimmed);
                (name, "Implementation".to_string())
            } else if is_python && (trimmed.starts_with("def ") || trimmed.starts_with("async def ")) {
                let kw = if trimmed.starts_with("async def ") { "async def " } else { "def " };
                (Self::extract_symbol_name(trimmed, kw), "Function".to_string())
            } else if is_python && trimmed.starts_with("class ") {
                (Self::extract_symbol_name(trimmed, "class "), "Class".to_string())
            } else {
                continue;
            };

            if name.is_empty() {
                continue;
            }

            let end_line = if is_python {
                let base_indent = line.len() - line.trim_start().len();
                let mut end = idx;
                for (j, l) in lines.iter().enumerate().skip(idx + 1) {
                    if l.trim().is_empty() || l.trim().starts_with('#') {
                        continue;
                    }
                    let indent = l.len() - l.trim_start().len();
                    if indent <= base_indent {
                        break;
                    }
                    end = j;
                }
                end
            } else {
                let mut brace_depth: i32 = 0;
                let mut saw_open_brace = false;
                let mut end = idx;
                for (j, l) in lines.iter().enumerate().skip(idx) {
                    let (open_count, close_count, has_semicolon) = Self::count_braces(l);
                    if open_count > 0 {
                        saw_open_brace = true;
                    }
                    brace_depth += open_count - close_count;
                    end = j;
                    if saw_open_brace && brace_depth <= 0 {
                        break;
                    }
                    if !saw_open_brace && has_semicolon {
                        break;
                    }
                }
                end
            };

            symbols.push(AstSymbolBlock {
                name,
                kind,
                signature: trimmed.to_string(),
                start_line: idx,
                end_line,
            });
        }

        symbols
    }

    /// Checks if a string contains an identifier as a distinct token (not a substring of a larger identifier)
    pub fn contains_identifier(text: &str, ident: &str) -> bool {
        if ident.is_empty() || !text.contains(ident) {
            return false;
        }

        let bytes = text.as_bytes();
        let ident_bytes = ident.as_bytes();
        let mut i = 0;
        while i + ident_bytes.len() <= bytes.len() {
            if &bytes[i..i + ident_bytes.len()] == ident_bytes {
                let prev_ok = i == 0 || (!bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'_');
                let next_pos = i + ident_bytes.len();
                let next_ok = next_pos == bytes.len()
                    || (!bytes[next_pos].is_ascii_alphanumeric() && bytes[next_pos] != b'_');
                if prev_ok && next_ok {
                    return true;
                }
            }
            i += 1;
        }
        false
    }

    fn extract_symbol_name(line: &str, keyword: &str) -> String {
        if let Some(pos) = line.find(keyword) {
            let rest = &line[pos + keyword.len()..];
            rest.split(|c: char| !c.is_alphanumeric() && c != '_')
                .next()
                .unwrap_or("")
                .to_string()
        } else {
            String::new()
        }
    }

    fn extract_impl_target(line: &str) -> String {
        if let Some(for_pos) = line.find(" for ") {
            let rest = line[for_pos + 5..].trim_start();
            rest.split(|c: char| !c.is_alphanumeric() && c != '_')
                .next()
                .unwrap_or("")
                .to_string()
        } else if let Some(impl_pos) = line.find("impl") {
            let mut rest = line[impl_pos + 4..].trim_start();
            if rest.starts_with('<')
                && let Some(close) = rest.find('>')
            {
                rest = rest[close + 1..].trim_start();
            }
            rest.split(|c: char| !c.is_alphanumeric() && c != '_')
                .next()
                .unwrap_or("")
                .to_string()
        } else {
            String::new()
        }
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

    #[test]
    fn test_slice_python_function() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("script.py");
        let file_str = file_path.to_str().unwrap();

        let py_code = r#"
def calculate_metrics(data):
    # Process items
    total = sum(data)
    avg = total / len(data) if data else 0
    return {
        'total': total,
        'avg': avg
    }

def helper_func():
    pass
"#;
        fs::write(&file_path, py_code).unwrap();

        let slice = AstTool::slice_symbol(file_str, "calculate_metrics").unwrap();
        assert!(slice.contains("def calculate_metrics(data):"));
        assert!(slice.contains("'total': total"));
        assert!(slice.contains("'avg': avg"));
        assert!(!slice.contains("def helper_func"));

        let not_found = AstTool::slice_symbol(file_str, "non_existent");
        assert!(not_found.is_err());
    }

    #[test]
    fn test_slice_generic_impl() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("generic.rs");
        let file_str = file_path.to_str().unwrap();

        let code = r#"
pub struct Container<T> {
    item: T,
}

impl<T: std::fmt::Display> std::fmt::Display for Container<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Item: {}", self.item)
    }
}
"#;
        fs::write(&file_path, code).unwrap();

        let slice = AstTool::slice_symbol(file_str, "Container").unwrap();
        assert!(slice.contains("pub struct Container<T>"));

        let slice_impl = AstTool::slice_symbol(file_str, "fmt").unwrap();
        assert!(slice_impl.contains("fn fmt(&self"));
        assert!(slice_impl.contains("write!(f, \"Item: {}\", self.item)"));
    }

    #[test]
    fn test_extract_dependency_slice_rust() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("pipeline.rs");
        let file_str = file_path.to_str().unwrap();

        let code = r#"
use std::sync::Arc;
use anyhow::Result;

pub struct Config {
    pub timeout_ms: u64,
}

pub struct Pipeline {
    pub config: Config,
}

impl Pipeline {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn process(&self, input: &str) -> Result<String> {
        let clean = self.sanitize(input);
        let transformed = self.transform(&clean);
        Ok(transformed)
    }

    fn sanitize(&self, raw: &str) -> String {
        raw.trim().to_string()
    }

    fn transform(&self, text: &str) -> String {
        format!("PROCESSED: {}", text)
    }
}

pub fn run_worker(p: &Pipeline, data: &str) -> Result<String> {
    p.process(data)
}
"#;
        fs::write(&file_path, code).unwrap();

        let slice = AstTool::extract_dependency_slice(file_str, "process").unwrap();
        assert!(slice.contains("AST DEPENDENCY CONTEXT SLICE: 'process'"));
        assert!(slice.contains("[Target Definition]"));
        assert!(slice.contains("pub fn process(&self, input: &str) -> Result<String>"));

        // Associated Types
        assert!(slice.contains("[Associated Types, Structs & Traits]"));
        assert!(slice.contains("Pipeline"));

        // Callees
        assert!(slice.contains("[Callee Signatures (Invoked by 'process')]"));
        assert!(slice.contains("fn sanitize"));
        assert!(slice.contains("fn transform"));

        // Callers
        assert!(slice.contains("[Caller Signatures (Calls 'process')]"));
        assert!(slice.contains("pub fn run_worker"));

        // Imports
        assert!(slice.contains("[Imports & Scope Context]"));
        assert!(slice.contains("use anyhow::Result;"));

        // Test via execute with action "dependency_slice"
        let args = serde_json::json!({
            "path": file_str,
            "action": "dependency_slice",
            "symbol": "process"
        });
        let exec_out = AstTool::execute(&args).unwrap();
        assert!(exec_out.contains("AST DEPENDENCY CONTEXT SLICE: 'process'"));
    }

    #[test]
    fn test_extract_dependency_slice_python() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("service.py");
        let file_str = file_path.to_str().unwrap();

        let py_code = r#"
import os
import json

def fetch_raw_data():
    return {"status": "ok"}

def handle_request(req):
    data = fetch_raw_data()
    return format_output(data)

def format_output(val):
    return json.dumps(val)

def main():
    handle_request({})
"#;
        fs::write(&file_path, py_code).unwrap();

        let slice = AstTool::extract_dependency_slice(file_str, "handle_request").unwrap();
        assert!(slice.contains("AST DEPENDENCY CONTEXT SLICE: 'handle_request'"));
        assert!(slice.contains("def handle_request(req):"));
        assert!(slice.contains("fetch_raw_data"));
        assert!(slice.contains("format_output"));
        assert!(slice.contains("def main():"));
        assert!(slice.contains("import json"));
    }
}
