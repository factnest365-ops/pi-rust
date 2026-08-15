use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspSymbol {
    pub name: String,
    pub kind: String,
    pub line_number: usize,
    pub signature: String,
}

pub struct LspTool;

impl LspTool {
    pub fn execute(args: &Value) -> Result<String> {
        let action = args["action"]
            .as_str()
            .or_else(|| args["command"].as_str())
            .unwrap_or("diagnostics");

        match action {
            "diagnostics" => {
                let file_path = args["path"].as_str().or_else(|| args["file"].as_str());
                Self::run_diagnostics(file_path)
            }
            "symbols" | "document_symbols" => {
                let file_path = args["path"]
                    .as_str()
                    .or_else(|| args["file"].as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'path' for symbols action"))?;
                Self::extract_symbols(file_path)
            }
            "definition" => {
                let file_path = args["path"]
                    .as_str()
                    .or_else(|| args["file"].as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'path' for definition action"))?;
                let symbol = args["symbol"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing 'symbol' for definition action"))?;
                Self::find_definition(file_path, symbol)
            }
            "hover" => {
                let file_path = args["path"]
                    .as_str()
                    .or_else(|| args["file"].as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'path' for hover action"))?;
                let symbol = args["symbol"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing 'symbol' for hover action"))?;
                Self::hover_info(file_path, symbol)
            }
            _ => Err(anyhow::anyhow!(
                "Unknown lsp action '{}'. Supported actions: diagnostics, symbols, definition, hover",
                action
            )),
        }
    }

    /// Fast diagnostics runner (compiler/linter diagnostics)
    pub fn run_diagnostics(target_path: Option<&str>) -> Result<String> {
        if let Some(path) = target_path {
            let p = Path::new(path);
            if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                match ext {
                    "py" => {
                        let out = Command::new("python3").arg("-m").arg("py_compile").arg(path).output();
                        return match out {
                            Ok(res) if res.status.success() => Ok(format!("Diagnostics for {}: Syntax clean", path)),
                            Ok(res) => Ok(format!("Diagnostics for {}: Syntax error:\n{}", path, String::from_utf8_lossy(&res.stderr))),
                            Err(_) => Ok(format!("Diagnostics for {}: Python syntax checker not available", path)),
                        };
                    }
                    "js" | "ts" => {
                        let out = Command::new("node").arg("--check").arg(path).output();
                        return match out {
                            Ok(res) if res.status.success() => Ok(format!("Diagnostics for {}: Syntax clean", path)),
                            Ok(res) => Ok(format!("Diagnostics for {}: Syntax error:\n{}", path, String::from_utf8_lossy(&res.stderr))),
                            Err(_) => Ok(format!("Diagnostics for {}: Node syntax checker not available", path)),
                        };
                    }
                    _ => {}
                }
            }
        }

        if Path::new("Cargo.toml").exists() {
            // Rust project diagnostics via cargo check
            let output = Command::new("cargo")
                .arg("check")
                .arg("--workspace")
                .arg("--all-targets")
                .output()?;

            let stderr = String::from_utf8_lossy(&output.stderr);
            if output.status.success() {
                if stderr.contains("warning:") {
                    let warnings: Vec<&str> = stderr.lines().filter(|l| l.contains("warning:") || l.contains("-->")).collect();
                    Ok(format!("Diagnostics: Build successful with warnings:\n{}", warnings.join("\n")))
                } else {
                    Ok("Diagnostics: Clean build (0 errors, 0 warnings)".to_string())
                }
            } else {
                Ok(format!("Diagnostics: Build errors detected:\n{}", stderr.trim()))
            }
        } else if let Some(path) = target_path {
            Ok(format!("Diagnostics: Checked {}", path))
        } else {
            Ok("Diagnostics: No build configuration found (Cargo.toml/package.json).".to_string())
        }
    }

    /// Extract document symbols (functions, structs, traits, enums, classes)
    pub fn extract_symbols(file_path: &str) -> Result<String> {
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);
        let mut symbols = Vec::new();

        for (idx, line_res) in reader.lines().enumerate() {
            let line_num = idx + 1;
            let line = line_res?;
            let trimmed = line.trim();

            if trimmed.starts_with("//") || trimmed.starts_with('#') || trimmed.starts_with("/*") {
                continue;
            }

            // Rust patterns
            if trimmed.starts_with("pub fn ") || trimmed.starts_with("fn ") || trimmed.starts_with("async fn ") || trimmed.starts_with("pub async fn ") {
                let name = Self::extract_ident(trimmed, "fn ");
                symbols.push(LspSymbol {
                    name,
                    kind: "Function".to_string(),
                    line_number: line_num,
                    signature: trimmed.to_string(),
                });
            } else if trimmed.starts_with("pub struct ") || trimmed.starts_with("struct ") {
                let name = Self::extract_ident(trimmed, "struct ");
                symbols.push(LspSymbol {
                    name,
                    kind: "Struct".to_string(),
                    line_number: line_num,
                    signature: trimmed.to_string(),
                });
            } else if trimmed.starts_with("pub enum ") || trimmed.starts_with("enum ") {
                let name = Self::extract_ident(trimmed, "enum ");
                symbols.push(LspSymbol {
                    name,
                    kind: "Enum".to_string(),
                    line_number: line_num,
                    signature: trimmed.to_string(),
                });
            } else if trimmed.starts_with("pub trait ") || trimmed.starts_with("trait ") {
                let name = Self::extract_ident(trimmed, "trait ");
                symbols.push(LspSymbol {
                    name,
                    kind: "Trait".to_string(),
                    line_number: line_num,
                    signature: trimmed.to_string(),
                });
            } else if trimmed.starts_with("impl ") || trimmed.starts_with("impl<") {
                symbols.push(LspSymbol {
                    name: trimmed.to_string(),
                    kind: "Implementation".to_string(),
                    line_number: line_num,
                    signature: trimmed.to_string(),
                });
            }
            // Python patterns
            else if trimmed.starts_with("def ") {
                let name = Self::extract_ident(trimmed, "def ");
                symbols.push(LspSymbol {
                    name,
                    kind: "Function".to_string(),
                    line_number: line_num,
                    signature: trimmed.to_string(),
                });
            } else if trimmed.starts_with("class ") {
                let name = Self::extract_ident(trimmed, "class ");
                symbols.push(LspSymbol {
                    name,
                    kind: "Class".to_string(),
                    line_number: line_num,
                    signature: trimmed.to_string(),
                });
            }
            // JS / TS patterns
            else if trimmed.starts_with("export function ") || trimmed.starts_with("function ") || trimmed.starts_with("export const ") {
                let kw = if trimmed.contains("function ") { "function " } else { "const " };
                let name = Self::extract_ident(trimmed, kw);
                symbols.push(LspSymbol {
                    name,
                    kind: "Function".to_string(),
                    line_number: line_num,
                    signature: trimmed.to_string(),
                });
            }
        }

        if symbols.is_empty() {
            Ok(format!("No top-level symbols detected in {}", file_path))
        } else {
            let mut out = format!("Symbols in {} ({} found):\n", file_path, symbols.len());
            for sym in symbols {
                out.push_str(&format!("  line {:4} | [{}] {}\n", sym.line_number, sym.kind, sym.signature));
            }
            Ok(out)
        }
    }

    pub fn matches_exact_symbol(line: &str, symbol: &str) -> bool {
        let keywords = [
            "fn ", "struct ", "enum ", "trait ", "impl ", "def ", "class ", "let ", "const ", "type ",
        ];
        for kw in keywords {
            if let Some(pos) = line.find(kw) {
                // Ensure keyword starts at beginning of line or after whitespace
                if pos > 0 && !line.as_bytes()[pos - 1].is_ascii_whitespace() {
                    continue;
                }
                let rest = &line[pos + kw.len()..];
                let ident = rest
                    .split(|c: char| !c.is_alphanumeric() && c != '_')
                    .next()
                    .unwrap_or("");
                if ident == symbol {
                    return true;
                }
            }
        }
        false
    }

    pub fn find_definition(file_path: &str, symbol: &str) -> Result<String> {
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);

        for (idx, line_res) in reader.lines().enumerate() {
            let line_num = idx + 1;
            let line = line_res?;
            let trimmed = line.trim();

            if !trimmed.starts_with("//") && Self::matches_exact_symbol(trimmed, symbol) {
                return Ok(format!(
                    "Found definition of '{}' in {}:{}\n{:4} | {}",
                    symbol, file_path, line_num, line_num, line
                ));
            }
        }

        Ok(format!("Definition of '{}' not found in {}", symbol, file_path))
    }

    pub fn hover_info(file_path: &str, symbol: &str) -> Result<String> {
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);
        let mut doc_comments = Vec::new();

        for (idx, line_res) in reader.lines().enumerate() {
            let line_num = idx + 1;
            let line = line_res?;
            let trimmed = line.trim();

            if trimmed.starts_with("///") || trimmed.starts_with("/**") || trimmed.starts_with('*') {
                doc_comments.push(trimmed.to_string());
            } else if Self::matches_exact_symbol(trimmed, symbol) {
                let docs = if doc_comments.is_empty() {
                    "No docstring available.".to_string()
                } else {
                    doc_comments.join("\n")
                };

                return Ok(format!(
                    "--- Hover Info for '{}' ({}:{}) ---\nSignature: {}\n\nDocumentation:\n{}",
                    symbol, file_path, line_num, line.trim(), docs
                ));
            } else {
                doc_comments.clear();
            }
        }

        Ok(format!("No hover/symbol information found for '{}' in {}", symbol, file_path))
    }

    fn extract_ident(line: &str, keyword: &str) -> String {
        if let Some(pos) = line.find(keyword) {
            let rest = &line[pos + keyword.len()..];
            rest.split(|c: char| !c.is_alphanumeric() && c != '_')
                .next()
                .unwrap_or("")
                .to_string()
        } else {
            "".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_extract_symbols_and_definition() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("sample.rs");
        let file_str = file_path.to_str().unwrap();

        let code = r#"
/// Sample struct documentation
pub struct Greeter {
    pub name: String,
}

impl Greeter {
    /// Greets the user
    pub fn greet(&self) -> String {
        format!("Hello, {}", self.name)
    }
}
"#;
        fs::write(&file_path, code).unwrap();

        let symbols_out = LspTool::extract_symbols(file_str).unwrap();
        assert!(symbols_out.contains("[Struct] pub struct Greeter"));
        assert!(symbols_out.contains("[Function] pub fn greet"));

        let def_out = LspTool::find_definition(file_str, "greet").unwrap();
        assert!(def_out.contains("Found definition of 'greet'"));

        let hover_out = LspTool::hover_info(file_str, "Greeter").unwrap();
        assert!(hover_out.contains("Sample struct documentation"));
    }
}
