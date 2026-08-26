use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const EMBEDDING_DIM: usize = 64;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoryEntry {
    pub id: String,
    pub scope: String,
    pub workspace_path: Option<String>,
    pub topic: String,
    pub content: String,
    pub counter_pattern: Option<String>,
    pub correct_pattern: Option<String>,
    pub embedding: Option<Vec<f32>>,
    pub valid_since: i64,
    pub valid_until: Option<i64>,
    pub access_count: i64,
    pub confidence: f64,
}

#[derive(Clone)]
pub struct TauVault {
    conn: Arc<Mutex<rusqlite::Connection>>,
    path: Option<PathBuf>,
}

impl std::fmt::Debug for TauVault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TauVault")
            .field("path", &self.path)
            .finish()
    }
}

impl Default for TauVault {
    fn default() -> Self {
        Self::new()
    }
}

impl TauVault {
    /// Creates or opens the default persistent memory vault at ~/.tau/vault.sqlite.
    /// Falls back to an in-memory database if file access fails.
    pub fn new() -> Self {
        Self::open_default().unwrap_or_else(|_| {
            Self::open_in_memory().expect("in-memory sqlite vault must always succeed")
        })
    }

    /// Opens or creates the default persistent database at ~/.tau/vault.sqlite.
    pub fn open_default() -> Result<Self> {
        let base_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".tau");
        std::fs::create_dir_all(&base_dir)?;
        let db_path = base_dir.join("vault.sqlite");
        Self::open(&db_path)
    }

    /// Opens or creates an SQLite database at a specific path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let p = path.as_ref().to_path_buf();
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = rusqlite::Connection::open(&p)?;
        Self::configure_connection(&conn)?;
        Self::init_schema(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            path: Some(p),
        })
    }

    /// Creates an ephemeral in-memory database (useful for isolated testing).
    pub fn open_in_memory() -> Result<Self> {
        let conn = rusqlite::Connection::open_in_memory()?;
        Self::configure_connection(&conn)?;
        Self::init_schema(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            path: None,
        })
    }

    fn configure_connection(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA foreign_keys = ON;
            "#,
        )?;
        Ok(())
    }

    fn init_schema(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS memories (
                id TEXT PRIMARY KEY,
                scope TEXT NOT NULL,
                workspace_path TEXT,
                topic TEXT NOT NULL,
                content TEXT NOT NULL,
                counter_pattern TEXT,
                correct_pattern TEXT,
                embedding BLOB,
                valid_since INTEGER NOT NULL,
                valid_until INTEGER,
                access_count INTEGER DEFAULT 0,
                confidence REAL DEFAULT 1.0
            );

            CREATE INDEX IF NOT EXISTS idx_memories_scope ON memories(scope);
            CREATE INDEX IF NOT EXISTS idx_memories_valid_until ON memories(valid_until);
            CREATE INDEX IF NOT EXISTS idx_memories_topic ON memories(topic);

            CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
                topic,
                content,
                counter_pattern,
                correct_pattern,
                content='memories',
                content_rowid='rowid'
            );

            CREATE TRIGGER IF NOT EXISTS memories_ai AFTER INSERT ON memories BEGIN
                INSERT INTO memories_fts(rowid, topic, content, counter_pattern, correct_pattern)
                VALUES (new.rowid, new.topic, new.content, new.counter_pattern, new.correct_pattern);
            END;

            CREATE TRIGGER IF NOT EXISTS memories_ad AFTER DELETE ON memories BEGIN
                INSERT INTO memories_fts(memories_fts, rowid, topic, content, counter_pattern, correct_pattern)
                VALUES('delete', old.rowid, old.topic, old.content, old.counter_pattern, old.correct_pattern);
            END;

            CREATE TRIGGER IF NOT EXISTS memories_au AFTER UPDATE ON memories BEGIN
                INSERT INTO memories_fts(memories_fts, rowid, topic, content, counter_pattern, correct_pattern)
                VALUES('delete', old.rowid, old.topic, old.content, old.counter_pattern, old.correct_pattern);
                INSERT INTO memories_fts(rowid, topic, content, counter_pattern, correct_pattern)
                VALUES (new.rowid, new.topic, new.content, new.counter_pattern, new.correct_pattern);
            END;
            "#,
        )?;
        Ok(())
    }

    /// Record a memory with scope, topic, content, optional counter/correct patterns, and optional embedding.
    pub fn record_memory(
        &self,
        scope: &str,
        topic: &str,
        content: &str,
        counter_pattern: Option<&str>,
        correct_pattern: Option<&str>,
        embedding: Option<&[f32]>,
    ) -> Result<String> {
        let current_ws = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .ok();
        self.record_memory_with_workspace(
            scope,
            current_ws.as_deref(),
            topic,
            content,
            counter_pattern,
            correct_pattern,
            embedding,
        )
    }

    /// Record a memory with explicit workspace path.
    #[allow(clippy::too_many_arguments)]
    pub fn record_memory_with_workspace(
        &self,
        scope: &str,
        workspace_path: Option<&str>,
        topic: &str,
        content: &str,
        counter_pattern: Option<&str>,
        correct_pattern: Option<&str>,
        embedding: Option<&[f32]>,
    ) -> Result<String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("SQLite lock error: {e}"))?;
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();

        let emb_bytes = match embedding {
            Some(emb) => f32_slice_to_bytes(emb),
            None => f32_slice_to_bytes(&compute_text_embedding(content)),
        };

        conn.execute(
            r#"
            INSERT INTO memories (
                id, scope, workspace_path, topic, content, counter_pattern, correct_pattern,
                embedding, valid_since, valid_until, access_count, confidence
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, 0, 1.0)
            "#,
            rusqlite::params![
                id,
                scope,
                workspace_path,
                topic,
                content,
                counter_pattern,
                correct_pattern,
                emb_bytes,
                now,
            ],
        )?;

        Ok(id)
    }

    /// Record an anti-pattern counter-rule distilled from a failure.
    pub fn record_counter_rule(
        &self,
        topic: &str,
        bad_pattern: &str,
        fix_pattern: &str,
    ) -> Result<String> {
        let content = format!("Avoid {bad_pattern} -> Instead {fix_pattern}");
        self.record_memory(
            "counter_rule",
            topic,
            &content,
            Some(bad_pattern),
            Some(fix_pattern),
            None,
        )
    }

    /// Record an episodic memory of a completed workflow.
    pub fn record_episodic_memory(&self, topic: &str, summary: &str) -> Result<String> {
        self.record_memory("episodic", topic, summary, None, None, None)
    }

    /// Revise belief: marks older memories matching `old_topic` as superseded (sets valid_until)
    /// and inserts the newly revised memory.
    pub fn revise_belief(&self, old_topic: &str, new_content: &str) -> Result<usize> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("SQLite lock error: {e}"))?;
        let now = chrono::Utc::now().timestamp();

        // Mark older matching active memories as superseded
        let updated = conn.execute(
            "UPDATE memories SET valid_until = ? WHERE topic = ? AND (valid_until IS NULL OR valid_until > ?)",
            rusqlite::params![now, old_topic, now],
        )?;

        let new_id = uuid::Uuid::new_v4().to_string();
        let current_ws = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .ok();
        let embedding = compute_text_embedding(new_content);
        let emb_bytes = f32_slice_to_bytes(&embedding);

        conn.execute(
            r#"
            INSERT INTO memories (
                id, scope, workspace_path, topic, content, counter_pattern, correct_pattern,
                embedding, valid_since, valid_until, access_count, confidence
            ) VALUES (?, 'workspace', ?, ?, ?, NULL, NULL, ?, ?, NULL, 0, 1.0)
            "#,
            rusqlite::params![new_id, current_ws, old_topic, new_content, emb_bytes, now,],
        )?;

        Ok(updated)
    }

    /// Hybrid Reciprocal Rank Fusion (RRF) search:
    /// Executes BM25 full-text matching + SIMD cosine ranking with recency decay.
    pub fn search_hybrid(&self, query: &str, limit: usize) -> Result<Vec<MemoryEntry>> {
        self.search_hybrid_with_embedding(query, None, limit)
    }

    /// Hybrid search with optional precomputed query embedding.
    pub fn search_hybrid_with_embedding(
        &self,
        query: &str,
        query_embedding: Option<&[f32]>,
        limit: usize,
    ) -> Result<Vec<MemoryEntry>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("SQLite lock error: {e}"))?;
        let now = chrono::Utc::now().timestamp();

        // 1. Fetch active candidate memories
        let mut stmt = conn.prepare(
            r#"
            SELECT id, scope, workspace_path, topic, content, counter_pattern, correct_pattern,
                   embedding, valid_since, valid_until, access_count, confidence
            FROM memories
            WHERE valid_until IS NULL OR valid_until > ?
            ORDER BY valid_since DESC
            "#,
        )?;

        let candidate_rows = stmt.query_map(rusqlite::params![now], |row| {
            let emb_blob: Option<Vec<u8>> = row.get(7)?;
            let emb_vec = emb_blob.map(|b| bytes_to_f32_vec(&b));

            Ok(MemoryEntry {
                id: row.get(0)?,
                scope: row.get(1)?,
                workspace_path: row.get(2)?,
                topic: row.get(3)?,
                content: row.get(4)?,
                counter_pattern: row.get(5)?,
                correct_pattern: row.get(6)?,
                embedding: emb_vec,
                valid_since: row.get(8)?,
                valid_until: row.get(9)?,
                access_count: row.get(10)?,
                confidence: row.get(11)?,
            })
        })?;

        let mut candidates: Vec<MemoryEntry> = Vec::new();
        for mem in candidate_rows.flatten() {
            candidates.push(mem);
        }

        if candidates.is_empty() {
            return Ok(Vec::new());
        }

        // 2. BM25 Search with FTS5
        let mut bm25_ranks: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let sanitized = sanitize_fts5_query(query);
        if !sanitized.is_empty()
            && let Ok(mut fts_stmt) = conn.prepare(
                r#"
                SELECT m.id, bm25(memories_fts) as rank
                FROM memories_fts
                JOIN memories m ON memories_fts.rowid = m.rowid
                WHERE memories_fts MATCH ? AND (m.valid_until IS NULL OR m.valid_until > ?)
                ORDER BY rank ASC
                LIMIT 50
                "#,
            )
            && let Ok(rows) = fts_stmt.query_map(rusqlite::params![sanitized, now], |row| {
                let id: String = row.get(0)?;
                Ok(id)
            })
        {
            for (rank_idx, r) in rows.flatten().enumerate() {
                bm25_ranks.insert(r, rank_idx + 1);
            }
        }

        // 3. SIMD Cosine Search
        let q_emb: Vec<f32> = match query_embedding {
            Some(emb) => emb.to_vec(),
            None => compute_text_embedding(query),
        };

        let mut simd_scores: Vec<(String, f32)> = Vec::new();
        for c in &candidates {
            let sim = if let Some(ref emb) = c.embedding {
                cosine_similarity(&q_emb, emb)
            } else {
                let emb = compute_text_embedding(&c.content);
                cosine_similarity(&q_emb, &emb)
            };
            if sim > 0.0 {
                simd_scores.push((c.id.clone(), sim));
            }
        }
        simd_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let mut simd_ranks: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for (idx, (id, _)) in simd_scores.into_iter().enumerate() {
            simd_ranks.insert(id, idx + 1);
        }

        // 4. RRF Ranking with Recency Decay
        // RRF(d) = (w_bm25 / (60 + r_bm25)) + (w_simd / (60 + r_simd)) * Decay(dt)
        let w_bm25 = 1.0f64;
        let w_simd = 1.0f64;
        let default_rank = 1000.0f64;

        let mut scored_candidates: Vec<(f64, MemoryEntry)> = candidates
            .into_iter()
            .map(|mem| {
                let r_bm25 = bm25_ranks
                    .get(&mem.id)
                    .copied()
                    .map(|r| r as f64)
                    .unwrap_or(default_rank);
                let r_simd = simd_ranks
                    .get(&mem.id)
                    .copied()
                    .map(|r| r as f64)
                    .unwrap_or(default_rank);

                let dt = (now - mem.valid_since).max(0) as f64;
                let decay = 1.0 / (1.0 + (dt / (86400.0 * 30.0))); // 30-day half-life

                let score = ((w_bm25 / (60.0 + r_bm25)) + (w_simd / (60.0 + r_simd)) * decay)
                    * mem.confidence;
                (score, mem)
            })
            .collect();

        scored_candidates
            .sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let top_entries: Vec<MemoryEntry> = scored_candidates
            .into_iter()
            .take(limit)
            .map(|(_, mem)| mem)
            .collect();

        // 5. Update access count
        for mem in &top_entries {
            let _ = conn.execute(
                "UPDATE memories SET access_count = access_count + 1 WHERE id = ?",
                rusqlite::params![mem.id],
            );
        }

        Ok(top_entries)
    }

    /// Formats top matching memories and rules into the `[Hindsight Memory & Rules]` prompt block.
    pub fn format_hindsight_prompt(&self, query: &str) -> String {
        let limit = 5;
        let memories = self.search_hybrid(query, limit).unwrap_or_default();
        if memories.is_empty() {
            return String::new();
        }

        let mut out = String::from("[Hindsight Memory & Rules]\n");
        for mem in memories {
            match mem.scope.as_str() {
                "counter_rule" => {
                    let bad = mem.counter_pattern.as_deref().unwrap_or("error");
                    let fix = mem.correct_pattern.as_deref().unwrap_or(&mem.content);
                    out.push_str(&format!(
                        "- [Counter-Rule] ({}): Avoid {} -> Instead {}\n",
                        mem.topic, bad, fix
                    ));
                }
                "workspace" => {
                    out.push_str(&format!(
                        "- [Workspace Rule] ({}): {}\n",
                        mem.topic, mem.content
                    ));
                }
                "global" => {
                    out.push_str(&format!(
                        "- [Global Rule] ({}): {}\n",
                        mem.topic, mem.content
                    ));
                }
                "episodic" => {
                    out.push_str(&format!(
                        "- [Episodic Memory] ({}): {}\n",
                        mem.topic, mem.content
                    ));
                }
                _ => {
                    out.push_str(&format!(
                        "- [{}] ({}): {}\n",
                        mem.scope, mem.topic, mem.content
                    ));
                }
            }
        }
        out.push_str("[End Hindsight Memory]");
        out
    }

    /// Lists all active memories up to `limit`.
    pub fn list_active_memories(&self, limit: usize) -> Result<Vec<MemoryEntry>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("SQLite lock error: {e}"))?;
        let now = chrono::Utc::now().timestamp();

        let mut stmt = conn.prepare(
            r#"
            SELECT id, scope, workspace_path, topic, content, counter_pattern, correct_pattern,
                   embedding, valid_since, valid_until, access_count, confidence
            FROM memories
            WHERE valid_until IS NULL OR valid_until > ?
            ORDER BY valid_since DESC
            LIMIT ?
            "#,
        )?;

        let rows = stmt.query_map(rusqlite::params![now, limit as i64], |row| {
            let emb_blob: Option<Vec<u8>> = row.get(7)?;
            let emb_vec = emb_blob.map(|b| bytes_to_f32_vec(&b));

            Ok(MemoryEntry {
                id: row.get(0)?,
                scope: row.get(1)?,
                workspace_path: row.get(2)?,
                topic: row.get(3)?,
                content: row.get(4)?,
                counter_pattern: row.get(5)?,
                correct_pattern: row.get(6)?,
                embedding: emb_vec,
                valid_since: row.get(8)?,
                valid_until: row.get(9)?,
                access_count: row.get(10)?,
                confidence: row.get(11)?,
            })
        })?;

        let mut entries = Vec::new();
        for entry in rows.flatten() {
            entries.push(entry);
        }
        Ok(entries)
    }

    /// Returns the count of active memories.
    pub fn count_active_memories(&self) -> Result<usize> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("SQLite lock error: {e}"))?;
        let now = chrono::Utc::now().timestamp();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM memories WHERE valid_until IS NULL OR valid_until > ?",
            rusqlite::params![now],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// Deletes a memory by ID.
    pub fn delete_memory(&self, id: &str) -> Result<bool> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("SQLite lock error: {e}"))?;
        let rows = conn.execute("DELETE FROM memories WHERE id = ?", rusqlite::params![id])?;
        Ok(rows > 0)
    }
}

/// Continuous Reflexion Engine for turning tool failures, compiler diagnostics,
/// and turn exceptions into permanent counter-rules and lessons.
pub struct ReflexionEngine;

impl ReflexionEngine {
    /// Distills a failed tool execution into a targeted counter-rule.
    pub fn distill_tool_failure(
        vault: &TauVault,
        tool_name: &str,
        arguments: &serde_json::Value,
        error_output: &str,
    ) -> Option<String> {
        let topic = match tool_name {
            "edit" => "tool_failure:edit",
            "write" => "tool_failure:write",
            "read" => "tool_failure:read",
            "bash" => {
                if error_output.contains("error[E") || error_output.contains("cargo check") {
                    "compiler_failure:rust"
                } else if error_output.contains("SyntaxError") || error_output.contains("TypeError")
                {
                    "script_failure"
                } else {
                    "tool_failure:bash"
                }
            }
            "git" => "tool_failure:git",
            "grep" => "tool_failure:grep",
            _ => "tool_failure:general",
        };

        let bad_pattern = if let Some(cmd) = arguments.get("command").and_then(|v| v.as_str()) {
            format!(
                "Command failed: {}",
                cmd.chars().take(80).collect::<String>()
            )
        } else if let Some(target) = arguments.get("target").and_then(|v| v.as_str()) {
            format!(
                "Edit target failed: {}",
                target.chars().take(60).collect::<String>()
            )
        } else if let Some(path) = arguments.get("path").and_then(|v| v.as_str()) {
            format!("Path operation failed: {}", path)
        } else {
            format!(
                "Tool {} failed with arguments: {}",
                tool_name,
                arguments.to_string().chars().take(60).collect::<String>()
            )
        };

        let fix_pattern = if error_output.contains("Target content not found") {
            "Verify file content with read tool before applying surgical edit".to_string()
        } else if error_output.contains("Multiple occurrences") {
            "Provide wider target context including surrounding lines to ensure unique replacement"
                .to_string()
        } else if error_output.contains("No such file or directory") {
            "Verify file existence with find or ls before reading or modifying".to_string()
        } else if error_output.contains("floor_char_boundary")
            || error_output.contains("byte index")
        {
            "Never slice UTF-8 strings by raw byte indices; always use floor_char_boundary"
                .to_string()
        } else {
            let first_err_line = error_output
                .lines()
                .find(|l| {
                    l.contains("error")
                        || l.contains("Error")
                        || l.contains("FAILED")
                        || l.contains("fail")
                })
                .unwrap_or_else(|| error_output.lines().next().unwrap_or(""));
            let trimmed = first_err_line.trim();
            if !trimmed.is_empty() {
                format!(
                    "Address root cause: {}",
                    trimmed.chars().take(120).collect::<String>()
                )
            } else {
                "Inspect tool parameters and preconditions before retrying".to_string()
            }
        };

        vault
            .record_counter_rule(topic, &bad_pattern, &fix_pattern)
            .ok()
    }

    /// Distills a turn-level error or model exception into a counter-rule.
    pub fn distill_turn_error(
        vault: &TauVault,
        user_input: &str,
        error_message: &str,
    ) -> Option<String> {
        let topic = "turn_failure";
        let bad_pattern = format!(
            "Turn error on prompt: {}",
            user_input.chars().take(80).collect::<String>()
        );
        let fix_pattern = format!(
            "Recover from turn error: {}",
            error_message.chars().take(120).collect::<String>()
        );
        vault
            .record_counter_rule(topic, &bad_pattern, &fix_pattern)
            .ok()
    }
}

/// Computes a deterministic 64-dimensional feature hash embedding for text in pure Rust.
pub fn compute_text_embedding(text: &str) -> Vec<f32> {
    let mut vec = vec![0.0f32; EMBEDDING_DIM];
    let lower = text.to_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();

    if words.is_empty() {
        return vec;
    }

    for word in &words {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash(word, &mut hasher);
        let h = std::hash::Hasher::finish(&hasher) as usize;
        let idx = h % EMBEDDING_DIM;
        let sign = if (h >> 8) & 1 == 0 { 1.0f32 } else { -1.0f32 };
        vec[idx] += sign;

        // Subword trigrams
        let chars: Vec<char> = word.chars().collect();
        if chars.len() >= 3 {
            for window in chars.windows(3) {
                let trigram: String = window.iter().collect();
                let mut tri_hasher = std::collections::hash_map::DefaultHasher::new();
                std::hash::Hash::hash(&trigram, &mut tri_hasher);
                let tri_h = std::hash::Hasher::finish(&tri_hasher) as usize;
                let tri_idx = tri_h % EMBEDDING_DIM;
                let tri_sign = if (tri_h >> 8) & 1 == 0 {
                    0.5f32
                } else {
                    -0.5f32
                };
                vec[tri_idx] += tri_sign;
            }
        }
    }

    // L2 Normalize
    let norm_sq: f32 = vec.iter().map(|v| v * v).sum();
    let norm = norm_sq.sqrt();
    if norm > 1e-8 {
        for v in vec.iter_mut() {
            *v /= norm;
        }
    }

    vec
}

/// Pure Rust SIMD/vectorized cosine similarity.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom > 1e-8 { dot / denom } else { 0.0 }
}

/// Serializes float slice into little-endian bytes.
pub fn f32_slice_to_bytes(slice: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(slice.len() * 4);
    for &val in slice {
        bytes.extend_from_slice(&val.to_le_bytes());
    }
    bytes
}

/// Deserializes little-endian bytes into float vector.
pub fn bytes_to_f32_vec(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

/// Sanitizes query string for SQLite FTS5 matching.
fn sanitize_fts5_query(raw: &str) -> String {
    let tokens: Vec<String> = raw
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{}\"*", w.replace('"', "\"\"")))
        .collect();

    if tokens.is_empty() {
        String::new()
    } else {
        tokens.join(" OR ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sqlite_init_in_memory() {
        let vault = TauVault::open_in_memory().expect("in-memory vault init");
        assert_eq!(vault.count_active_memories().unwrap(), 0);
    }

    #[test]
    fn test_record_and_retrieve_memory() {
        let vault = TauVault::open_in_memory().unwrap();
        let id = vault
            .record_memory(
                "workspace",
                "rust_standards",
                "Always check clippy warnings before committing",
                None,
                None,
                None,
            )
            .unwrap();

        assert!(!id.is_empty());
        assert_eq!(vault.count_active_memories().unwrap(), 1);

        let active = vault.list_active_memories(10).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].topic, "rust_standards");
        assert_eq!(active[0].scope, "workspace");
        assert!(active[0].embedding.is_some());
    }

    #[test]
    fn test_record_counter_rule() {
        let vault = TauVault::open_in_memory().unwrap();
        let id = vault
            .record_counter_rule(
                "string_slicing",
                "&s[..len] raw byte index slicing",
                "s.floor_char_boundary(len) boundary safety",
            )
            .unwrap();

        assert!(!id.is_empty());
        let active = vault.list_active_memories(10).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].scope, "counter_rule");
        assert_eq!(
            active[0].counter_pattern.as_deref(),
            Some("&s[..len] raw byte index slicing")
        );
        assert_eq!(
            active[0].correct_pattern.as_deref(),
            Some("s.floor_char_boundary(len) boundary safety")
        );
    }

    #[test]
    fn test_fts5_bm25_search() {
        let vault = TauVault::open_in_memory().unwrap();
        vault
            .record_memory(
                "workspace",
                "database",
                "Postgres connection pooling using r2d2",
                None,
                None,
                None,
            )
            .unwrap();
        vault
            .record_memory(
                "workspace",
                "auth",
                "JWT authentication token refresh rotation",
                None,
                None,
                None,
            )
            .unwrap();

        let results = vault.search_hybrid("postgres r2d2", 5).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].topic, "database");

        let auth_results = vault.search_hybrid("jwt token", 5).unwrap();
        assert!(!auth_results.is_empty());
        assert_eq!(auth_results[0].topic, "auth");
    }

    #[test]
    fn test_simd_cosine_ranking() {
        let v1 = compute_text_embedding("neural network transformer training");
        let v2 = compute_text_embedding("neural network deep learning models");
        let v3 = compute_text_embedding("banana apple fruit recipe");

        let sim_close = cosine_similarity(&v1, &v2);
        let sim_far = cosine_similarity(&v1, &v3);

        assert!(sim_close > sim_far);
        assert!(sim_close > 0.3);
    }

    #[test]
    fn test_hybrid_search_and_recency_decay() {
        let vault = TauVault::open_in_memory().unwrap();
        vault
            .record_counter_rule(
                "edit_disambiguation",
                "Multiple occurrences of target in edit tool",
                "Provide wider line context for exact single target match",
            )
            .unwrap();
        vault
            .record_episodic_memory(
                "refactor_session",
                "Successfully refactored session DAG tree nodes",
            )
            .unwrap();

        let res = vault
            .search_hybrid("edit tool occurrences match", 5)
            .unwrap();
        assert!(!res.is_empty());
        assert_eq!(res[0].topic, "edit_disambiguation");
    }

    #[test]
    fn test_belief_revision() {
        let vault = TauVault::open_in_memory().unwrap();
        vault
            .record_memory(
                "workspace",
                "architecture_rule",
                "Old architecture: Use monolith src/ folder",
                None,
                None,
                None,
            )
            .unwrap();

        assert_eq!(vault.count_active_memories().unwrap(), 1);

        // Revise belief
        let updated = vault
            .revise_belief(
                "architecture_rule",
                "New architecture: 7 decoupled crates under crates/",
            )
            .unwrap();

        assert_eq!(updated, 1);
        // Active memory count should still be 1 because old is superseded
        assert_eq!(vault.count_active_memories().unwrap(), 1);

        let active = vault.list_active_memories(10).unwrap();
        assert_eq!(active.len(), 1);
        assert!(active[0].content.contains("7 decoupled crates"));
    }

    #[test]
    fn test_format_hindsight_prompt() {
        let vault = TauVault::open_in_memory().unwrap();
        assert_eq!(vault.format_hindsight_prompt("rust"), "");

        vault
            .record_counter_rule(
                "unicode_slicing",
                "Raw byte indexing &s[..len]",
                "Use floor_char_boundary",
            )
            .unwrap();

        let prompt = vault.format_hindsight_prompt("unicode slicing byte index");
        assert!(prompt.contains("[Hindsight Memory & Rules]"));
        assert!(prompt.contains("[Counter-Rule] (unicode_slicing)"));
        assert!(
            prompt.contains("Avoid Raw byte indexing &s[..len] -> Instead Use floor_char_boundary")
        );
        assert!(prompt.contains("[End Hindsight Memory]"));
    }

    #[test]
    fn test_reflexion_engine_distillation() {
        let vault = TauVault::open_in_memory().unwrap();

        // Simulate failed edit tool invocation
        let args = serde_json::json!({
            "path": "src/lib.rs",
            "target": "fn main()"
        });
        let err_output = "Error: Multiple occurrences of target found in src/lib.rs";

        let rule_id = ReflexionEngine::distill_tool_failure(&vault, "edit", &args, err_output);
        assert!(rule_id.is_some());

        let active = vault.list_active_memories(5).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].topic, "tool_failure:edit");
        assert!(
            active[0]
                .correct_pattern
                .as_ref()
                .unwrap()
                .contains("wider target context")
        );

        // Simulate turn error
        let turn_rule_id = ReflexionEngine::distill_turn_error(
            &vault,
            "Deploy to production",
            "API rate limit exceeded: 429 Too Many Requests",
        );
        assert!(turn_rule_id.is_some());
        assert_eq!(vault.count_active_memories().unwrap(), 2);
    }

    #[test]
    fn test_fts5_triggers_sync() {
        let vault = TauVault::open_in_memory().unwrap();
        let id = vault
            .record_memory(
                "workspace",
                "temporary_rule",
                "Delete this rule soon",
                None,
                None,
                None,
            )
            .unwrap();

        let found = vault.search_hybrid("temporary rule", 5).unwrap();
        assert_eq!(found.len(), 1);

        // Delete the memory and verify FTS5 table is synchronized
        let deleted = vault.delete_memory(&id).unwrap();
        assert!(deleted);
        assert_eq!(vault.count_active_memories().unwrap(), 0);

        let search_after_delete = vault.search_hybrid("temporary rule", 5).unwrap();
        assert!(search_after_delete.is_empty());
    }
}
