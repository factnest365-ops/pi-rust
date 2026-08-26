use std::{
    collections::{HashMap, HashSet},
    fmt,
    path::{Path, PathBuf},
};

use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SymbolId(pub u64);

impl fmt::Display for SymbolId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub id: SymbolId,
    pub name: String,
    pub kind: SymbolKind,
    pub file: PathBuf,
    pub line_range: (usize, usize),
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Struct,
    Enum,
    Trait,
    Class,
    Method,
    Module,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum EdgeKind {
    Calls,
    Imports,
    Implements,
    Contains,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Edge {
    pub from: SymbolId,
    pub to: SymbolId,
    pub kind: EdgeKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRegion {
    pub file: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SliceResult {
    pub symbols: Vec<Symbol>,
    pub files: Vec<FileRegion>,
    pub edges: Vec<Edge>,
}

impl SliceResult {
    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty() && self.files.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Upstream,
    Downstream,
    Both,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CodeGraph {
    pub symbols: Vec<Symbol>,
    pub edges: Vec<Edge>,
    pub by_id: HashMap<SymbolId, usize>,
    pub name_index: HashMap<String, Vec<usize>>,
    pub file_index: HashMap<PathBuf, Vec<usize>>,
}

impl CodeGraph {
    pub fn new() -> Self {
        Self {
            symbols: Vec::new(),
            edges: Vec::new(),
            by_id: HashMap::new(),
            name_index: HashMap::new(),
            file_index: HashMap::new(),
        }
    }
}

impl Default for CodeGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeGraph {
    pub fn add_symbol(&mut self, symbol: Symbol) -> SymbolId {
        let id = symbol.id;
        let idx = self.symbols.len();
        self.by_id.insert(id, idx);
        self.name_index
            .entry(symbol.name.clone())
            .or_default()
            .push(idx);
        self.file_index
            .entry(symbol.file.clone())
            .or_default()
            .push(idx);
        self.symbols.push(symbol);
        id
    }

    pub fn add_edge(&mut self, edge: Edge) {
        self.edges.push(edge);
    }

    pub fn get(&self, id: SymbolId) -> Option<&Symbol> {
        self.by_id.get(&id).map(|idx| &self.symbols[*idx])
    }

    pub fn by_name(&self, name: &str) -> &[usize] {
        self.name_index
            .get(name)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn in_file(&self, file: &Path) -> &[usize] {
        self.file_index
            .get(file)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn index_workspace(root: &Path, languages: &[Language]) -> anyhow::Result<Self> {
        let mut graph = Self::new();
        let walker = WorkspaceWalker::new(root);

        for path in walker {
            let lang = match Language::from_path(path.as_path()) {
                Ok(lang) => lang,
                Err(_) => continue,
            };
            if !languages.contains(&lang) {
                continue;
            }
            let text = match std::fs::read_to_string(path.as_path()) {
                Ok(text) => text,
                Err(_) => continue,
            };
            let symbols = match extract(lang, path.as_path(), &text) {
                Ok(symbols) => symbols,
                Err(_) => continue,
            };
            for symbol in symbols {
                let _ = graph.add_symbol(symbol);
            }
        }

        graph.build_heuristic_edges()?;
        Ok(graph)
    }

    pub fn reindex_file(&mut self, path: &Path, text: &str) {
        self.remove_file(path);
        let Ok(lang) = Language::from_path(path) else {
            return;
        };
        let Ok(symbols) = extract(lang, path, text) else {
            return;
        };
        for symbol in symbols {
            let _ = self.add_symbol(symbol);
        }
        let _ = self.build_heuristic_edges();
    }

    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    fn remove_file(&mut self, path: &Path) {
        if let Some(idxs) = self.file_index.remove(path) {
            let mut remove_ids = HashSet::new();
            for idx in idxs {
                let sym = &self.symbols[idx];
                remove_ids.insert(sym.id);
                self.name_index
                    .entry(sym.name.clone())
                    .or_default()
                    .retain(|i| *i != idx);
            }

            self.symbols.retain(|sym| !remove_ids.contains(&sym.id));
            self.by_id.retain(|id, _| !remove_ids.contains(id));
            self.edges
                .retain(|edge| !remove_ids.contains(&edge.from) && !remove_ids.contains(&edge.to));
        }
    }

    fn build_heuristic_edges(&mut self) -> anyhow::Result<()> {
        let import_re = Regex::new(r"(?m)^\s*(?:use|import)\s+(.+)$")?;
        let call_re = Regex::new(r#"\b([A-Za-z_][A-Za-z0-9_]*)\s*\("#)?;
        let trait_re = Regex::new(r#"(?m)^\s*(?:impl|trait)\s+([A-Za-z_][A-Za-z0-9_:<>]+)"#)?;
        let self_method_re = Regex::new(r#"(?m)self\s*\.\s*([A-Za-z_][A-Za-z0-9_]*)\s*\("#)?;

        let mut pending: Vec<Edge> = Vec::new();

        for sym in &self.symbols {
            let Ok(text) = std::fs::read_to_string(&sym.file) else {
                continue;
            };
            let lines: Vec<&str> = text.lines().collect();
            let start = sym.line_range.0.saturating_sub(1);
            let end = sym.line_range.1.min(lines.len());

            for line in &lines[start..end] {
                let Some(cap) = import_re.captures(line) else {
                    continue;
                };
                let Some(target) =
                    resolve_import_target(&cap[1], sym.file.as_path(), &self.file_index)
                else {
                    continue;
                };
                let Some(target_idxs) = self.file_index.get(&target) else {
                    continue;
                };
                for idx in target_idxs {
                    pending.push(Edge {
                        from: sym.id,
                        to: self.symbols[*idx].id,
                        kind: EdgeKind::Imports,
                    });
                }
                if let Some(cap) = trait_re.captures(line) {
                    let target_name = cap[1].split('<').next().unwrap_or(&cap[1]).trim();
                    for idx in self.by_name(target_name) {
                        let target = &self.symbols[*idx];
                        if target.file == sym.file {
                            pending.push(Edge {
                                from: sym.id,
                                to: target.id,
                                kind: EdgeKind::Implements,
                            });
                        }
                    }
                }

                for cap in call_re.captures_iter(line) {
                    let name = &cap[1];
                    if name == "self" {
                        continue;
                    }
                    for idx in self.by_name(name) {
                        let target = &self.symbols[*idx];
                        if target.file == sym.file {
                            pending.push(Edge {
                                from: sym.id,
                                to: target.id,
                                kind: EdgeKind::Calls,
                            });
                        }
                    }
                }

                for cap in self_method_re.captures_iter(line) {
                    let name = &cap[1];
                    for idx in self.by_name(name) {
                        let target = &self.symbols[*idx];
                        if target.file == sym.file {
                            pending.push(Edge {
                                from: sym.id,
                                to: target.id,
                                kind: EdgeKind::Calls,
                            });
                        }
                    }
                }
            }
        }

        for edge in pending {
            self.add_edge(edge);
        }

        Ok(())
    }
}

fn resolve_import_target(
    raw: &str,
    from: &Path,
    file_index: &HashMap<PathBuf, Vec<usize>>,
) -> Option<PathBuf> {
    let raw = raw.split(';').next().unwrap_or(raw).trim();
    let module = raw.split("::").next().unwrap_or(raw).trim();
    if module.is_empty() {
        return None;
    }

    let mut candidates = Vec::new();
    candidates.push(from.with_file_name(format!("{}.rs", module)));
    if let Some(parent) = from.parent() {
        candidates.push(parent.join(module).join("mod.rs"));
        candidates.push(parent.join(format!("{}.rs", module)));
    }
    candidates.push(from.with_file_name(format!("{}.ts", module)));
    candidates.push(from.with_file_name(format!("{}.tsx", module)));
    candidates.push(from.with_file_name(format!("{}.js", module)));

    for cand in candidates {
        if file_index.contains_key(&cand) {
            return Some(cand);
        }
    }
    None
}

struct WorkspaceWalker<'a> {
    _root: &'a Path,
    stack: Vec<PathBuf>,
}

impl<'a> WorkspaceWalker<'a> {
    fn new(root: &'a Path) -> Self {
        Self {
            _root: root,
            stack: vec![root.to_path_buf()],
        }
    }
}

impl<'a> Iterator for WorkspaceWalker<'a> {
    type Item = PathBuf;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(path) = self.stack.pop() {
            let metadata = match std::fs::metadata(&path) {
                Ok(metadata) => metadata,
                Err(_) => continue,
            };
            if metadata.is_dir() {
                let name = match path.file_name() {
                    Some(name) => name.to_string_lossy(),
                    None => continue,
                };
                if name.starts_with('.')
                    || name == "target"
                    || name == "node_modules"
                    || name == ".git"
                {
                    continue;
                }
                if metadata.len() > 1_048_576 {
                    continue;
                }
                let mut children = Vec::new();
                if let Ok(entries) = std::fs::read_dir(&path) {
                    for entry in entries.flatten() {
                        children.push(entry.path());
                    }
                }
                children.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
                self.stack.extend(children.into_iter().rev());
            } else if metadata.is_file() {
                return Some(path);
            }
        }
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    Rust,
    Python,
    TypeScript,
    JavaScript,
    Unknown,
}

impl Language {
    pub fn from_path(path: &Path) -> anyhow::Result<Self> {
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        match ext {
            "rs" => Ok(Self::Rust),
            "py" => Ok(Self::Python),
            "ts" => Ok(Self::TypeScript),
            "tsx" | "js" | "jsx" => Ok(Self::JavaScript),
            _ => anyhow::bail!("unsupported extension"),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::TypeScript => "typescript",
            Self::JavaScript => "javascript",
            Self::Unknown => "unknown",
        }
    }
}

fn extract(lang: Language, path: &Path, text: &str) -> anyhow::Result<Vec<Symbol>> {
    match lang {
        Language::Rust => extract_rust(path, text),
        Language::Python => extract_python(path, text),
        Language::TypeScript | Language::JavaScript => extract_ts_js(path, text),
        Language::Unknown => Ok(Vec::new()),
    }
}

fn extract_rust(path: &Path, text: &str) -> anyhow::Result<Vec<Symbol>> {
    let mut symbols = Vec::new();
    let mut id_counter: u64 = 0;
    let mut next_id = || {
        id_counter = id_counter.wrapping_add(1);
        SymbolId(id_counter)
    };

    let struct_re = Regex::new(r"(?m)^\s*(?:pub\s+)?struct\s+([A-Z][A-Za-z0-9_]*)\b")?;
    let enum_re = Regex::new(r"(?m)^\s*(?:pub\s+)?enum\s+([A-Z][A-Za-z0-9_]*)\b")?;
    let trait_re = Regex::new(r"(?m)^\s*(?:pub\s+)?trait\s+([A-Z][A-Za-z0-9_]*)\b")?;
    let fn_re = Regex::new(r"(?m)^\s*(?:pub\s+)?(?:async\s+)?fn\s+([A-Za-z0-9_]+)\b")?;
    let impl_re = Regex::new(
        r"(?m)^\s*impl(?:\s*<[^>]*>)?\s+(?:([A-Za-z0-9_:<>]+)\s+for\s+)?([A-Z][A-Za-z0-9_]*)\b",
    )?;

    let lines: Vec<usize> = text.lines().map(|l| l.len()).collect();
    let mut line_starts = vec![0usize];
    for len in lines {
        let next = *line_starts.last().unwrap() + len + 1;
        line_starts.push(next);
    }

    for cap in struct_re.captures_iter(text) {
        let start = line_for_offset(&line_starts, cap.get(0).unwrap().start());
        let end = line_for_offset(&line_starts, cap.get(0).unwrap().end());
        symbols.push(Symbol {
            id: next_id(),
            name: cap[1].trim().to_string(),
            kind: SymbolKind::Struct,
            file: path.to_path_buf(),
            line_range: (start, end),
            signature: None,
        });
    }

    for cap in enum_re.captures_iter(text) {
        let start = line_for_offset(&line_starts, cap.get(0).unwrap().start());
        let end = line_for_offset(&line_starts, cap.get(0).unwrap().end());
        symbols.push(Symbol {
            id: next_id(),
            name: cap[1].trim().to_string(),
            kind: SymbolKind::Enum,
            file: path.to_path_buf(),
            line_range: (start, end),
            signature: None,
        });
    }

    for cap in trait_re.captures_iter(text) {
        let start = line_for_offset(&line_starts, cap.get(0).unwrap().start());
        let end = line_for_offset(&line_starts, cap.get(0).unwrap().end());
        symbols.push(Symbol {
            id: next_id(),
            name: cap[1].trim().to_string(),
            kind: SymbolKind::Trait,
            file: path.to_path_buf(),
            line_range: (start, end),
            signature: None,
        });
    }

    for cap in impl_re.captures_iter(text) {
        let target = cap[1].trim();
        let ty = cap[2].trim();
        let start = line_for_offset(&line_starts, cap.get(0).unwrap().start());
        let end = line_for_offset(&line_starts, cap.get(0).unwrap().end());
        let kind = if target.is_empty() {
            SymbolKind::Struct
        } else {
            SymbolKind::Method
        };
        let name = if target.is_empty() {
            ty.to_string()
        } else {
            target.to_string()
        };
        symbols.push(Symbol {
            id: next_id(),
            name,
            kind,
            file: path.to_path_buf(),
            line_range: (start, end),
            signature: None,
        });
    }

    for cap in fn_re.captures_iter(text) {
        let name = cap[1].trim();
        let start = line_for_offset(&line_starts, cap.get(0).unwrap().start());
        let end = line_for_offset(&line_starts, cap.get(0).unwrap().end());
        symbols.push(Symbol {
            id: next_id(),
            name: name.to_string(),
            kind: SymbolKind::Function,
            file: path.to_path_buf(),
            line_range: (start, end),
            signature: None,
        });
    }

    Ok(symbols)
}

fn extract_python(path: &Path, text: &str) -> anyhow::Result<Vec<Symbol>> {
    let mut symbols = Vec::new();
    let mut id_counter: u64 = 0;
    let mut next_id = || {
        id_counter = id_counter.wrapping_add(1);
        SymbolId(id_counter)
    };

    let class_re = Regex::new(r"(?m)^\s*class\s+([A-Za-z_][A-Za-z0-9_]*)\b")?;
    let def_re = Regex::new(r"(?m)^\s*def\s+([A-Za-z_][A-Za-z0-9_]*)\b")?;
    let import_re = Regex::new(r"(?m)^\s*import\s+([A-Za-z0-9_.]+)")?;
    let from_import_re =
        Regex::new(r"(?m)^\s*from\s+([A-Za-z0-9_.]+)\s+import\s+([A-Za-z0-9_ ,]+)")?;

    let lines: Vec<usize> = text.lines().map(|l| l.len()).collect();
    let mut line_starts = vec![0usize];
    for len in lines {
        let next = *line_starts.last().unwrap() + len + 1;
        line_starts.push(next);
    }

    for cap in class_re.captures_iter(text) {
        let start = line_for_offset(&line_starts, cap.get(0).unwrap().start());
        let end = line_for_offset(&line_starts, cap.get(0).unwrap().end());
        symbols.push(Symbol {
            id: next_id(),
            name: cap[1].trim().to_string(),
            kind: SymbolKind::Class,
            file: path.to_path_buf(),
            line_range: (start, end),
            signature: None,
        });
    }

    for cap in def_re.captures_iter(text) {
        let start = line_for_offset(&line_starts, cap.get(0).unwrap().start());
        let end = line_for_offset(&line_starts, cap.get(0).unwrap().end());
        symbols.push(Symbol {
            id: next_id(),
            name: cap[1].trim().to_string(),
            kind: SymbolKind::Function,
            file: path.to_path_buf(),
            line_range: (start, end),
            signature: None,
        });
    }

    for cap in import_re.captures_iter(text) {
        let start = line_for_offset(&line_starts, cap.get(0).unwrap().start());
        let end = line_for_offset(&line_starts, cap.get(0).unwrap().end());
        symbols.push(Symbol {
            id: next_id(),
            name: cap[1].trim().to_string(),
            kind: SymbolKind::Module,
            file: path.to_path_buf(),
            line_range: (start, end),
            signature: None,
        });
    }

    for cap in from_import_re.captures_iter(text) {
        let start = line_for_offset(&line_starts, cap.get(0).unwrap().start());
        let end = line_for_offset(&line_starts, cap.get(0).unwrap().end());
        symbols.push(Symbol {
            id: next_id(),
            name: cap[1].trim().to_string(),
            kind: SymbolKind::Module,
            file: path.to_path_buf(),
            line_range: (start, end),
            signature: None,
        });
    }

    Ok(symbols)
}

fn extract_ts_js(path: &Path, text: &str) -> anyhow::Result<Vec<Symbol>> {
    let mut symbols = Vec::new();
    let mut id_counter: u64 = 0;
    let mut next_id = || {
        id_counter = id_counter.wrapping_add(1);
        SymbolId(id_counter)
    };

    let class_re =
        Regex::new(r"(?m)^\s*(?:export\s+)?(?:abstract\s+)?class\s+([A-Z][A-Za-z0-9_]*)\b")?;
    let fn_re =
        Regex::new(r"(?m)^\s*(?:export\s+)?(?:async\s+)?function\s+([A-Za-z_][A-Za-z0-9_]*)\b")?;
    let arrow_re = Regex::new(
        r"(?m)(?:const|let|var)\s+([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(?:async\s+)?\([^)]*\)\s*=>",
    )?;
    let import_re = Regex::new(r#"(?m)^\s*import\s+.*?\s+from\s+(?:'[^']+'|"[^"]+")"#)?;

    let lines: Vec<usize> = text.lines().map(|l| l.len()).collect();
    let mut line_starts = vec![0usize];
    for len in lines {
        let next = *line_starts.last().unwrap() + len + 1;
        line_starts.push(next);
    }

    for cap in class_re.captures_iter(text) {
        let start = line_for_offset(&line_starts, cap.get(0).unwrap().start());
        let end = line_for_offset(&line_starts, cap.get(0).unwrap().end());
        symbols.push(Symbol {
            id: next_id(),
            name: cap[1].trim().to_string(),
            kind: SymbolKind::Class,
            file: path.to_path_buf(),
            line_range: (start, end),
            signature: None,
        });
    }

    for cap in fn_re.captures_iter(text) {
        let start = line_for_offset(&line_starts, cap.get(0).unwrap().start());
        let end = line_for_offset(&line_starts, cap.get(0).unwrap().end());
        symbols.push(Symbol {
            id: next_id(),
            name: cap[1].trim().to_string(),
            kind: SymbolKind::Function,
            file: path.to_path_buf(),
            line_range: (start, end),
            signature: None,
        });
    }

    for cap in arrow_re.captures_iter(text) {
        let start = line_for_offset(&line_starts, cap.get(0).unwrap().start());
        let end = line_for_offset(&line_starts, cap.get(0).unwrap().end());
        symbols.push(Symbol {
            id: next_id(),
            name: cap[1].trim().to_string(),
            kind: SymbolKind::Function,
            file: path.to_path_buf(),
            line_range: (start, end),
            signature: None,
        });
    }

    for cap in import_re.captures_iter(text) {
        let start = line_for_offset(&line_starts, cap.get(0).unwrap().start());
        let end = line_for_offset(&line_starts, cap.get(0).unwrap().end());
        symbols.push(Symbol {
            id: next_id(),
            name: cap[1].trim().to_string(),
            kind: SymbolKind::Module,
            file: path.to_path_buf(),
            line_range: (start, end),
            signature: None,
        });
    }

    Ok(symbols)
}

fn line_for_offset(line_starts: &[usize], offset: usize) -> usize {
    match line_starts.binary_search(&offset) {
        Ok(i) => i + 1,
        Err(i) => i,
    }
}
