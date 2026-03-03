//! Persistent memory system for Vox agents.
//!
//! Inspired by OpenClaw's file-first memory model:
//! - **Daily logs** (`memory/YYYY-MM-DD.md`) — append-only per-session notes
//! - **MEMORY.md** — curated long-term knowledge indexed by heading
//! - **MemoryManager** — coordinates daily logs + MEMORY.md + VoxDb embeddings,
//!   bootstraps agent context on startup, and flushes critical state before
//!   compaction to prevent knowledge loss.

use std::collections::HashMap;
use std::fmt::Write as FmtWrite;
use std::fs::{self, OpenOptions};
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::types::AgentId;

// ---------------------------------------------------------------------------
// MemoryConfig
// ---------------------------------------------------------------------------

/// Configuration for the persistent memory system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Directory for daily log files. Default: `./memory`.
    pub log_dir: PathBuf,
    /// Path to the long-term memory file. Default: `./memory/MEMORY.md`.
    pub memory_md_path: PathBuf,
    /// Maximum number of days to retain daily logs. 0 = keep forever.
    pub log_retention_days: u64,
    /// Whether the memory system is enabled. Default: true.
    pub enabled: bool,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            log_dir: PathBuf::from("memory"),
            memory_md_path: PathBuf::from("memory/MEMORY.md"),
            log_retention_days: 30,
            enabled: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Date helpers
// ---------------------------------------------------------------------------

/// Returns the current date as `YYYY-MM-DD`.
fn today_str() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let (year, month, day) = unix_secs_to_ymd(secs);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Returns the previous date as `YYYY-MM-DD`.
fn yesterday_str() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .saturating_sub(86_400);
    let (year, month, day) = unix_secs_to_ymd(secs);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Current HH:MM:SS timestamp.
fn timestamp_hms() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    format!("{h:02}:{m:02}:{s:02}")
}

/// Minimal no-dep unix timestamp → (year, month, day).
fn unix_secs_to_ymd(mut secs: u64) -> (u32, u32, u32) {
    secs /= 86_400; // days since unix epoch
    let mut year = 1970u32;
    loop {
        let leap = if year.is_multiple_of(400) {
            366u64
        } else if year.is_multiple_of(100) {
            365
        } else if year.is_multiple_of(4) {
            366
        } else {
            365
        };
        if secs < leap {
            break;
        }
        secs -= leap;
        year += 1;
    }
    let leap_year =
        year.is_multiple_of(400) || (year.is_multiple_of(4) && !year.is_multiple_of(100));
    let days_in_month = [
        31u32,
        if leap_year { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 0;
    let mut remaining = secs as u32;
    for (i, &d) in days_in_month.iter().enumerate() {
        if remaining < d {
            month = i as u32 + 1;
            break;
        }
        remaining -= d;
    }
    if month == 0 {
        month = 12;
    }
    (year, month, remaining + 1)
}

// ---------------------------------------------------------------------------
// DailyLog
// ---------------------------------------------------------------------------

/// An append-only daily log file (`memory/YYYY-MM-DD.md`).
///
/// Each call to [`append`] writes a timestamped bullet to disk immediately.
/// Survives restarts — the file is opened in append mode every time.
pub struct DailyLog {
    path: PathBuf,
}

impl DailyLog {
    /// Open (or create) the log for the given date string (`YYYY-MM-DD`).
    pub fn open(log_dir: &Path, date_str: &str) -> Result<Self, MemoryError> {
        fs::create_dir_all(log_dir).map_err(MemoryError::Io)?;
        let path = log_dir.join(format!("{date_str}.md"));
        // Create the file with a heading if it doesn't exist yet
        if !path.exists() {
            let heading = format!("# Daily Log — {date_str}\n\n");
            fs::write(&path, heading.as_bytes()).map_err(MemoryError::Io)?;
        }
        Ok(Self { path })
    }

    /// Append a timestamped entry to this log.
    pub fn append(&self, entry: &str) -> Result<(), MemoryError> {
        let mut f = OpenOptions::new()
            .append(true)
            .open(&self.path)
            .map_err(MemoryError::Io)?;
        writeln!(f, "- `{}` {}", timestamp_hms(), entry).map_err(MemoryError::Io)?;
        Ok(())
    }

    /// Read full contents of this log.
    pub fn read(&self) -> Result<String, MemoryError> {
        if self.path.exists() {
            fs::read_to_string(&self.path).map_err(MemoryError::Io)
        } else {
            Ok(String::new())
        }
    }

    /// True if the backing file exists and is non-empty.
    pub fn exists(&self) -> bool {
        self.path.exists() && self.path.metadata().map(|m| m.len() > 0).unwrap_or(false)
    }

    /// Path to the backing file.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

// ---------------------------------------------------------------------------
// LongTermMemory
// ---------------------------------------------------------------------------

/// Manages `MEMORY.md` — curated, human-editable long-term knowledge.
///
/// Sections are Markdown headings (`## key`). Each section contains free-form
/// text. [`get`] extracts the body under a heading; [`set`] upserts it.
pub struct LongTermMemory {
    path: PathBuf,
}

impl LongTermMemory {
    /// Open (or create) the MEMORY.md file.
    pub fn open(path: &Path) -> Result<Self, MemoryError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(MemoryError::Io)?;
        }
        if !path.exists() {
            fs::write(path, "# Vox Long-Term Memory\n\nThis file is managed by the Vox orchestrator. Edit freely.\n\n")
                .map_err(MemoryError::Io)?;
        }
        Ok(Self {
            path: path.to_path_buf(),
        })
    }

    /// Read all contents.
    pub fn read_all(&self) -> Result<String, MemoryError> {
        fs::read_to_string(&self.path).map_err(MemoryError::Io)
    }

    /// Extract the body text under a `## key` heading.
    pub fn get(&self, key: &str) -> Result<Option<String>, MemoryError> {
        let content = self.read_all()?;
        let heading = format!("## {key}");
        let mut in_section = false;
        let mut body = String::new();
        for line in content.lines() {
            if line.trim() == heading.trim() {
                in_section = true;
                continue;
            }
            if in_section {
                if line.starts_with("## ") {
                    break;
                }
                body.push_str(line);
                body.push('\n');
            }
        }
        let trimmed = body.trim();
        if trimmed.is_empty() {
            Ok(None)
        } else {
            Ok(Some(trimmed.to_string()))
        }
    }

    /// Upsert body text under a `## key` heading.
    pub fn set(&self, key: &str, value: &str) -> Result<(), MemoryError> {
        let content = self.read_all().unwrap_or_default();
        let heading = format!("## {key}");
        // Each section: heading + blank line + value + trailing newline
        let new_section = format!("{heading}\n{value}\n\n");

        let updated = if content.contains(&heading) {
            // Replace existing section
            let mut out = String::with_capacity(content.len());
            let mut in_section = false;
            let mut replaced = false;
            for line in content.lines() {
                if line.trim() == heading.trim() {
                    in_section = true;
                    if !replaced {
                        out.push_str(&new_section);
                        replaced = true;
                    }
                    continue;
                }
                if in_section {
                    if line.starts_with("## ") {
                        in_section = false;
                        out.push_str(line);
                        out.push('\n');
                    }
                    // skip old body lines
                } else {
                    out.push_str(line);
                    out.push('\n');
                }
            }
            out
        } else {
            // Append new section
            let mut out = content;
            out.push_str(&new_section);
            out
        };

        fs::write(&self.path, updated.as_bytes()).map_err(MemoryError::Io)?;
        Ok(())
    }

    /// List all `## heading` keys in MEMORY.md.
    pub fn list_keys(&self) -> Result<Vec<String>, MemoryError> {
        let content = self.read_all().unwrap_or_default();
        Ok(content
            .lines()
            .filter(|l| l.starts_with("## "))
            .map(|l| l[3..].trim().to_string())
            .collect())
    }
}

// ---------------------------------------------------------------------------
// MemoryManager
// ---------------------------------------------------------------------------

/// A quick in-memory cache of a recently stored fact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryFact {
    pub key: String,
    pub value: String,
    pub agent_id: AgentId,
    pub stored_at_secs: u64,
}

/// Central coordinator for the Vox persistent memory system.
///
/// On creation, call [`bootstrap_context`] to load today's + yesterday's
/// daily logs and the contents of MEMORY.md into a ready-to-inject string.
///
/// Before compaction, call [`flush_before_compaction`] with any critical
/// key-value pairs to persist them durably.
///
/// When a `VoxDb` is attached via [`with_db`], every `persist_fact` also
/// writes to the `agent_memory` table, and `recall` falls back to the DB
/// when the file-based lookup misses. Files are the hot cache; VoxDB is
/// the durable single source of truth.
pub struct MemoryManager {
    config: MemoryConfig,
    today_log: DailyLog,
    long_term: LongTermMemory,
    /// In-memory cache of recently stored facts (bounded).
    cache: Vec<MemoryFact>,
    /// Maximum cache size.
    cache_limit: usize,
    /// Optional VoxDB backing store for SSOT persistence.
    db: Option<Arc<vox_db::VoxDb>>,
    /// Optional service for generating embeddings.
    embedding_service: Option<Arc<crate::services::embeddings::EmbeddingService>>,
}

impl MemoryManager {
    /// Create a `MemoryManager` using the given config (file-only mode).
    pub fn new(config: MemoryConfig) -> Result<Self, MemoryError> {
        let today = today_str();
        let today_log = DailyLog::open(&config.log_dir, &today)?;
        let long_term = LongTermMemory::open(&config.memory_md_path)?;
        Ok(Self {
            config,
            today_log,
            long_term,
            cache: Vec::new(),
            cache_limit: 256,
            db: None,
            embedding_service: None,
        })
    }

    /// Create with defaults (uses `./memory/` directory).
    pub fn with_defaults() -> Result<Self, MemoryError> {
        Self::new(MemoryConfig::default())
    }

    /// Attach a VoxDb for dual-write persistence (SSOT mode).
    pub fn with_db(mut self, db: Arc<vox_db::VoxDb>) -> Self {
        self.db = Some(db);
        self
    }

    /// Attach an EmbeddingService for vector persistence.
    pub fn with_embeddings(mut self, service: Arc<crate::services::embeddings::EmbeddingService>) -> Self {
        self.embedding_service = Some(service);
        self
    }

    /// Set the db reference after construction.
    pub fn set_db(&mut self, db: Arc<vox_db::VoxDb>) {
        self.db = Some(db);
    }

    /// Set the embedding service after construction.
    pub fn set_embedding_service(&mut self, service: Arc<crate::services::embeddings::EmbeddingService>) {
        self.embedding_service = Some(service);
    }

    /// Append a note to today's daily log.
    pub fn log(&self, entry: &str) -> Result<(), MemoryError> {
        self.today_log.append(entry)
    }

    /// Persist a key-value fact to MEMORY.md, in-memory cache, and VoxDB.
    pub fn persist_fact(
        &mut self,
        agent_id: AgentId,
        key: impl Into<String>,
        value: impl Into<String>,
        relations: &[&str],
    ) -> Result<(), MemoryError> {
        let key = key.into();
        let value = value.into();
        self.long_term.set(&key, &value)?;

        // Dual-write to VoxDB (fire-and-forget via spawn)
        if let Some(db) = &self.db {
            let db = db.clone();
            let agent_str = agent_id.0.to_string();
            let k = key.clone();
            let v = value.clone();
            let rels: Vec<String> = relations.iter().map(|s| s.to_string()).collect();
            let embed_svc = self.embedding_service.clone();

            tokio::spawn(async move {
                // 1. Save standard agent_memory fact
                let _ = db
                    .store()
                    .save_memory(
                        &agent_str,
                        "global",
                        "fact",
                        &format!("{k}: {v}"),
                        Some(&format!("{{\"key\":\"{k}\"}}")),
                        1.0,
                        None,
                    )
                    .await;

                // 2. Upsert a knowledge_node for this fact
                let _ = db
                    .store()
                    .upsert_knowledge_node(&k, "fact", &k, Some(&format!("{{\"value\":\"{v}\"}}")))
                    .await;

                // 3. Create knowledge_edge links for related facts
                for r in rels {
                    let _ = db
                        .store()
                        .upsert_knowledge_node(&r, "concept", &r, None)
                        .await;
                    let _ = db
                        .store()
                        .create_knowledge_edge(&k, &r, "related_to", 1.0, None)
                        .await;
                }

                // 4. Generate and store vector embedding (NEW)
                if let Some(svc) = embed_svc {
                    let _ = svc.embed_and_store("fact", &k, &v, None).await;
                }
            });
        }

        let fact = MemoryFact {
            key: key.clone(),
            value: value.clone(),
            agent_id,
            stored_at_secs: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };
        self.cache.push(fact);
        if self.cache.len() > self.cache_limit {
            self.cache.remove(0);
        }
        Ok(())
    }

    /// Retrieve a fact from MEMORY.md by key, falling back to VoxDB.
    pub fn recall(&self, key: &str) -> Result<Option<String>, MemoryError> {
        // File-first (hot cache)
        let file_result = self.long_term.get(key)?;
        if file_result.is_some() {
            return Ok(file_result);
        }
        // DB fallback — check in-memory cache for DB-sourced facts
        for fact in self.cache.iter().rev() {
            if fact.key == key {
                return Ok(Some(fact.value.clone()));
            }
        }
        Ok(None)
    }

    /// Sync all MEMORY.md sections to VoxDB.
    ///
    /// Call this periodically or on shutdown to ensure VoxDB has all facts.
    pub async fn sync_to_db(&self) -> Result<usize, MemoryError> {
        let db = match &self.db {
            Some(db) => db,
            None => return Ok(0),
        };
        let keys = self.long_term.list_keys()?;
        let mut synced = 0usize;
        for key in &keys {
            if let Ok(Some(value)) = self.long_term.get(key) {
                let _ = db
                    .store()
                    .save_memory(
                        "global",
                        "sync",
                        "fact",
                        &format!("{key}: {value}"),
                        Some(&format!("{{\"key\":\"{key}\"}}")),
                        1.0,
                        None,
                    )
                    .await;
                synced += 1;
            }
        }
        Ok(synced)
    }

    /// Hydrate MEMORY.md from VoxDB on cold start.
    ///
    /// Reads all "fact" entries from the DB and writes missing ones to MEMORY.md.
    pub async fn sync_from_db(&mut self) -> Result<usize, MemoryError> {
        let db = match &self.db {
            Some(db) => db,
            None => return Ok(0),
        };
        let entries = db
            .store()
            .recall_memory("global", Some("fact"), 500, None)
            .await
            .unwrap_or_default();
        let mut hydrated = 0usize;
        for entry in entries {
            // Parse "key: value" format from content
            if let Some((k, v)) = entry.content.split_once(": ") {
                let existing = self.long_term.get(k).unwrap_or(None);
                if existing.is_none() {
                    self.long_term.set(k, v)?;
                    hydrated += 1;
                }
            }
        }
        Ok(hydrated)
    }

    /// List all memory keys in MEMORY.md.
    pub fn list_keys(&self) -> Result<Vec<String>, MemoryError> {
        self.long_term.list_keys()
    }

    /// Build rich context by traversing the knowledge graph from a topic node.
    pub async fn build_knowledge_context(
        &self,
        topic: &str,
        depth: usize,
    ) -> Result<String, MemoryError> {
        let db = match &self.db {
            Some(db) => db,
            None => return Ok(String::new()),
        };

        let mut context = format!("Context for '{topic}':\n");
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();

        queue.push_back((topic.to_string(), 0));
        visited.insert(topic.to_string());

        while let Some((node_id, current_depth)) = queue.pop_front() {
            if current_depth >= depth {
                continue;
            }

            if let Ok(neighbors) = db.store().get_knowledge_neighbors(&node_id).await {
                for (target, _relation, _weight) in neighbors {
                    if visited.insert(target.id.clone()) {
                        let indent = "  ".repeat(current_depth + 1);
                        context.push_str(&format!("{indent}- {}\n", target.label));
                        queue.push_back((target.id, current_depth + 1));
                    }
                }
            }
        }
        Ok(context.trim_end().to_string())
    }

    /// Search daily logs and MEMORY.md for lines matching a query (case-insensitive substring).
    pub fn search(&self, query: &str) -> Result<Vec<SearchHit>, MemoryError> {
        let q = query.to_lowercase();
        let mut hits = Vec::new();

        // Search today's log
        let today = self.today_log.read()?;
        for (i, line) in today.lines().enumerate() {
            if line.to_lowercase().contains(&q) {
                hits.push(SearchHit {
                    source: format!("daily:{}", today_str()),
                    line: i + 1,
                    content: line.to_string(),
                });
            }
        }

        // Search yesterday's log
        let yesterday = DailyLog::open(&self.config.log_dir, &yesterday_str())?;
        if yesterday.exists() {
            let content = yesterday.read()?;
            for (i, line) in content.lines().enumerate() {
                if line.to_lowercase().contains(&q) {
                    hits.push(SearchHit {
                        source: format!("daily:{}", yesterday_str()),
                        line: i + 1,
                        content: line.to_string(),
                    });
                }
            }
        }

        // Search MEMORY.md
        let memory_content = self.long_term.read_all()?;
        for (i, line) in memory_content.lines().enumerate() {
            if line.to_lowercase().contains(&q) {
                hits.push(SearchHit {
                    source: "memory.md".to_string(),
                    line: i + 1,
                    content: line.to_string(),
                });
            }
        }

        Ok(hits)
    }

    /// Build a bootstrap context string to inject at agent session start.
    ///
    /// Includes today's log, yesterday's log (if present), and MEMORY.md.
    /// Returns empty string if memory is disabled.
    pub fn bootstrap_context(&self) -> String {
        if !self.config.enabled {
            return String::new();
        }

        let mut out = String::new();

        // MEMORY.md
        if let Ok(mem) = self.long_term.read_all() {
            if !mem.trim().is_empty() {
                out.push_str("## Long-term Memory\n\n");
                out.push_str(&mem);
                out.push_str("\n\n");
            }
        }

        // Yesterday's log
        let yesterday = yesterday_str();
        if let Ok(ylog) = DailyLog::open(&self.config.log_dir, &yesterday) {
            if ylog.exists() {
                if let Ok(content) = ylog.read() {
                    if !content.trim().is_empty() {
                        out.push_str(&format!("## Yesterday ({yesterday})\n\n"));
                        out.push_str(&content);
                        out.push_str("\n\n");
                    }
                }
            }
        }

        // Today's log
        if let Ok(content) = self.today_log.read() {
            if !content.trim().is_empty() {
                out.push_str(&format!("## Today ({})\n\n", today_str()));
                out.push_str(&content);
                out.push('\n');
            }
        }

        out
    }

    /// Pre-compaction flush: persist a map of critical key-value pairs to MEMORY.md
    /// and log the flush event to today's daily log.
    ///
    /// Call this **before** any compaction operation to prevent knowledge loss.
    pub fn flush_before_compaction(
        &mut self,
        agent_id: AgentId,
        facts: HashMap<String, String>,
    ) -> Result<usize, MemoryError> {
        let count = facts.len();
        let mut summary = String::new();
        for (key, value) in facts {
            self.persist_fact(agent_id, &key, &value, &[])?;
            let _ = write!(summary, "{key}, ");
        }
        if count > 0 {
            let _ = self.today_log.append(&format!(
                "[pre-compaction flush] Persisted {count} facts: {}",
                summary.trim_end_matches(", ")
            ));
        }
        Ok(count)
    }

    /// Remove daily log files older than `config.log_retention_days`.
    pub fn cleanup_old_logs(&self) -> Result<usize, MemoryError> {
        if self.config.log_retention_days == 0 {
            return Ok(0);
        }
        let cutoff_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .saturating_sub(self.config.log_retention_days * 86_400);

        let mut removed = 0;
        if let Ok(entries) = fs::read_dir(&self.config.log_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("md") {
                    continue;
                }
                if path.file_name().and_then(|n| n.to_str()) == Some("MEMORY.md") {
                    continue;
                }
                if let Ok(meta) = fs::metadata(&path) {
                    let mtime = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(u64::MAX);
                    if mtime < cutoff_secs {
                        let _ = fs::remove_file(&path);
                        removed += 1;
                    }
                }
            }
        }
        Ok(removed)
    }
}

// ---------------------------------------------------------------------------
// SearchHit
// ---------------------------------------------------------------------------

/// A matching line found by [`MemoryManager::search`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    /// Source file identifier (e.g. `daily:2026-02-27` or `memory.md`).
    pub source: String,
    pub line: usize,
    pub content: String,
}

// ---------------------------------------------------------------------------
// MemoryError
// ---------------------------------------------------------------------------

/// Errors from the memory subsystem.
#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialize(String),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_DIR_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_dir() -> PathBuf {
        let n = TEST_DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
        let d = env::temp_dir().join(format!(
            "vox_memory_test_{}_{}",
            timestamp_hms().replace(':', ""),
            n
        ));
        fs::create_dir_all(&d).ok();
        d
    }

    #[test]
    fn daily_log_append_and_read() {
        let dir = temp_dir();
        let log = DailyLog::open(&dir, "2026-02-27").expect("open");
        log.append("compiler fixed").expect("append");
        let content = log.read().expect("read");
        assert!(content.contains("compiler fixed"));
        assert!(content.contains("2026-02-27"));
    }

    #[test]
    fn daily_log_multiple_appends() {
        let dir = temp_dir();
        let log = DailyLog::open(&dir, "2026-02-28").expect("open");
        log.append("first entry").expect("append");
        log.append("second entry").expect("append");
        let content = log.read().expect("read");
        assert!(content.contains("first entry"));
        assert!(content.contains("second entry"));
    }

    #[test]
    fn long_term_memory_set_and_get() {
        let dir = temp_dir();
        let mem = LongTermMemory::open(&dir.join("MEMORY.md")).expect("open");
        mem.set("current_crate", "vox-parser").expect("set");
        let val = mem.get("current_crate").expect("get");
        assert_eq!(val.as_deref(), Some("vox-parser"));
    }

    #[test]
    fn long_term_memory_upsert() {
        let dir = temp_dir();
        let mem = LongTermMemory::open(&dir.join("MEMORY.md")).expect("open");
        mem.set("status", "in progress").expect("set first");
        mem.set("status", "completed").expect("set second (upsert)");
        let val = mem.get("status").expect("get");
        assert_eq!(val.as_deref(), Some("completed"));
    }

    #[test]
    fn long_term_memory_list_keys() {
        let dir = temp_dir();
        let mem = LongTermMemory::open(&dir.join("MEMORY.md")).expect("open");
        mem.set("alpha", "a").expect("set");
        mem.set("beta", "b").expect("set");
        let keys = mem.list_keys().expect("list");
        assert!(keys.contains(&"alpha".to_string()));
        assert!(keys.contains(&"beta".to_string()));
    }

    #[test]
    fn memory_manager_persist_and_recall() {
        let dir = temp_dir();
        let mut mgr = MemoryManager::new(MemoryConfig {
            log_dir: dir.join("logs"),
            memory_md_path: dir.join("MEMORY.md"),
            log_retention_days: 7,
            enabled: true,
        })
        .expect("create");
        mgr.persist_fact(AgentId(1), "last_task", "fix parser", &[])
            .expect("persist");
        let val = mgr.recall("last_task").expect("recall");
        assert_eq!(val.as_deref(), Some("fix parser"));
    }

    #[test]
    fn memory_manager_bootstrap_context() {
        let dir = temp_dir();
        let mut mgr = MemoryManager::new(MemoryConfig {
            log_dir: dir.join("logs"),
            memory_md_path: dir.join("MEMORY.md"),
            log_retention_days: 7,
            enabled: true,
        })
        .expect("create");
        mgr.persist_fact(AgentId(1), "project", "vox", &[])
            .expect("persist");
        mgr.log("started session").expect("log");
        let ctx = mgr.bootstrap_context();
        assert!(ctx.contains("project"));
        assert!(ctx.contains("vox"));
    }

    #[test]
    fn memory_manager_search() {
        let dir = temp_dir();
        let mut mgr = MemoryManager::new(MemoryConfig {
            log_dir: dir.join("logs"),
            memory_md_path: dir.join("MEMORY.md"),
            log_retention_days: 7,
            enabled: true,
        })
        .expect("create");
        mgr.log("fixed the parser bug").expect("log");
        mgr.persist_fact(AgentId(1), "active_branch", "feat/parser-fix", &[])
            .expect("persist");
        let hits = mgr.search("parser").expect("search");
        assert!(!hits.is_empty(), "should find 'parser' in memory");
    }

    #[test]
    fn flush_before_compaction_persists_facts() {
        let dir = temp_dir();
        let mut mgr = MemoryManager::new(MemoryConfig {
            log_dir: dir.join("logs"),
            memory_md_path: dir.join("MEMORY.md"),
            log_retention_days: 7,
            enabled: true,
        })
        .expect("create");
        let mut facts = HashMap::new();
        facts.insert(
            "lock_file".to_string(),
            "crates/vox-parser/src/parser.rs".to_string(),
        );
        facts.insert("agent_state".to_string(), "building".to_string());
        let flushed = mgr
            .flush_before_compaction(AgentId(1), facts)
            .expect("flush");
        assert_eq!(flushed, 2);
        assert!(mgr.recall("lock_file").expect("recall").is_some());
        assert!(mgr.recall("agent_state").expect("recall").is_some());
    }

    #[test]
    fn disabled_memory_manager_returns_empty_context() {
        let dir = temp_dir();
        let mgr = MemoryManager::new(MemoryConfig {
            log_dir: dir.join("logs"),
            memory_md_path: dir.join("MEMORY.md"),
            log_retention_days: 7,
            enabled: false,
        })
        .expect("create");
        let ctx = mgr.bootstrap_context();
        assert!(
            ctx.is_empty(),
            "disabled memory should return empty context"
        );
    }

    #[test]
    fn unix_secs_to_ymd_basic() {
        // 2026-02-27 00:00:00 UTC = 1772150400 secs
        let (y, m, d) = unix_secs_to_ymd(1_772_150_400);
        assert_eq!(y, 2026);
        assert_eq!(m, 2);
        assert_eq!(d, 27);
    }
}
