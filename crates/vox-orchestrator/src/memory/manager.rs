use std::collections::HashMap;
use std::fmt::Write as FmtWrite;
use std::fs;
use std::io::Write;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::types::AgentId;

use super::config::MemoryConfig;
use super::error::MemoryError;
use super::long_term::LongTermMemory;
use super::search_hit::SearchHit;
use super::time::today_str;

/// A quick in-memory cache of a recently stored fact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryFact {
    /// Section heading / fact key in MEMORY.md.
    pub key: String,
    /// Serialized fact body.
    pub value: String,
    /// Agent that persisted the fact.
    pub agent_id: AgentId,
    /// Unix seconds when the fact was stored.
    pub stored_at_secs: u64,
}

/// Central coordinator for the Vox persistent memory system.
///
/// On creation, call [`MemoryManager::bootstrap_context`] to load today's + yesterday's
/// daily logs and the contents of MEMORY.md into a ready-to-inject string.
///
/// Before compaction, call [`MemoryManager::flush_before_compaction`] with any critical
/// key-value pairs to persist them durably.
///
/// When a `VoxDb` is attached via [`MemoryManager::with_db`], every `persist_fact` also
/// writes to Codex `memories`. Recall order: **in-memory cache** (recent `persist_fact`),
/// **MEMORY.md**, then **Codex** (via [`Self::lookup_fact_by_key`] — sync [`Self::recall`] stops after file).
pub struct MemoryManager {
    pub(super) config: MemoryConfig,
    pub(super) long_term: LongTermMemory,
    /// In-memory cache of recently stored facts (bounded).
    pub(super) cache: Vec<MemoryFact>,
    /// Maximum cache size.
    pub(super) cache_limit: usize,
    /// Optional VoxDB backing store for SSOT persistence.
    pub(super) db: Option<Arc<vox_db::VoxDb>>,
    /// Optional service for generating embeddings.
    pub(super) embedding_service: Option<Arc<vox_search::EmbeddingService>>,
}

impl MemoryManager {
    /// Create a `MemoryManager` using the given config (file-only mode).
    pub fn new(config: MemoryConfig) -> Result<Self, MemoryError> {
        let long_term = LongTermMemory::open(&config.memory_md_path)?;
        Ok(Self {
            config,
            long_term,
            cache: Vec::new(),
            cache_limit: 256,
            db: None,
            embedding_service: None,
        })
    }

    /// Create with defaults (uses `./memory/` directory, account `"global"`).
    pub fn with_defaults() -> Result<Self, MemoryError> {
        Self::new(MemoryConfig::default())
    }

    /// Convenience factory for a specific account under `base_dir`.
    ///
    /// Equivalent to `MemoryManager::new(MemoryConfig::for_account(account_id, base_dir))`.
    pub fn for_account(
        account_id: impl Into<String>,
        base_dir: impl Into<std::path::PathBuf>,
    ) -> Result<Self, MemoryError> {
        Self::new(MemoryConfig::for_account(account_id, base_dir))
    }

    /// Return the `account_id` this manager is scoped to.
    pub fn account_id(&self) -> &str {
        &self.config.account_id
    }

    /// Attach a VoxDb for dual-write persistence (SSOT mode).
    pub fn with_db(mut self, db: Arc<vox_db::VoxDb>) -> Self {
        self.db = Some(db);
        self
    }

    /// Attach an EmbeddingService for vector persistence.
    pub fn with_embeddings(mut self, service: Arc<vox_search::EmbeddingService>) -> Self {
        self.embedding_service = Some(service);
        self
    }

    /// Set the db reference after construction.
    pub fn set_db(&mut self, db: Arc<vox_db::VoxDb>) {
        self.db = Some(db);
    }

    /// Set the embedding service after construction.
    pub fn set_embedding_service(&mut self, service: Arc<vox_search::EmbeddingService>) {
        self.embedding_service = Some(service);
    }

    /// Append a note to today's daily log.
    pub fn log(&self, entry: &str) -> Result<(), MemoryError> {
        let path = self.config.log_dir.join(format!("{}.md", today_str()));
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(MemoryError::Io)?;
        }
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(MemoryError::Io)?;
        writeln!(f, "{entry}").map_err(MemoryError::Io)?;
        f.sync_all().map_err(MemoryError::Io)?;

        if let Some(db) = &self.db {
            let db = db.clone();
            let entry = entry.to_string();
            let acc = self.config.account_id.clone();
            tokio::spawn(async move {
                let _ = db
                    .save_memory(vox_db::SaveMemoryParams {
                        agent_id: "global",
                        session_id: "global",
                        memory_type: "daily_log",
                        content: &entry,
                        metadata: Some(&format!("{{\"account_id\":\"{acc}\"}}")),
                        importance: 1.0,
                        vcs_snapshot_id: None,
                    })
                    .await;
            });
        }
        Ok(())
    }

    /// Persist a key-value fact to MEMORY.md, in-memory cache, and VoxDB.
    pub fn persist_fact(
        &mut self,
        agent_id: AgentId,
        key: impl Into<String>,
        value: impl Into<String>,
        relations: &[&str],
        media_url: Option<&str>,
        media_type: Option<&str>,
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
            let m_url = media_url.map(|s| s.to_string());
            let m_type = media_type.map(|s| s.to_string());
            let account_id_str = self.config.account_id.clone();

            tokio::spawn(async move {
                // 1. Save standard agent_memory fact (tagged with account_id for tenant filtering)
                let fact_line = format!("{k}: {v}");
                let fact_meta = format!(
                    "{{\"key\":\"{k}\",\"account_id\":\"{acc}\"}}",
                    acc = account_id_str
                );
                let _ = db
                    .save_memory(vox_db::SaveMemoryParams {
                        agent_id: &agent_str,
                        session_id: "global",
                        memory_type: "fact",
                        content: &fact_line,
                        metadata: Some(fact_meta.as_str()),
                        importance: 1.0,
                        vcs_snapshot_id: None,
                    })
                    .await;

                // 2. Upsert a knowledge_node for this fact
                let _ = db
                    .upsert_knowledge_node(
                        &k,
                        "fact",
                        &k,
                        Some(&format!("{{\"value\":\"{v}\"}}")),
                        m_url.as_deref(),
                        m_type.as_deref(),
                    )
                    .await;

                // 3. Create knowledge_edge links for related facts
                for r in rels {
                    let _ = db
                        .upsert_knowledge_node(&r, "concept", &r, None, None, None)
                        .await;
                    let _ = db
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

    /// Exact key lookup: **cache** → **MEMORY.md** → Codex `memories` (`global` / type `fact`).
    ///
    /// Used by MCP and other tooling that needs a durable key-value hit. Broader context retrieval
    /// should use the RAG / retrieval bundle path where appropriate.
    pub async fn lookup_fact_by_key(&self, key: &str) -> Result<Option<String>, MemoryError> {
        for fact in self.cache.iter().rev() {
            if fact.key == key {
                return Ok(Some(fact.value.clone()));
            }
        }
        if let Ok(Some(v)) = self.long_term.get(key) {
            return Ok(Some(v));
        }
        let Some(db) = &self.db else {
            return Ok(None);
        };
        let entries = db
            .recall_memory("global", Some("fact"), 500, None)
            .await
            .unwrap_or_default();
        for entry in entries {
            if let Some((k, v)) = entry.content.split_once(": ") {
                if k == key {
                    tracing::debug!(
                        target: "vox_orchestrator::memory",
                        key,
                        "lookup_fact_by_key: hit Codex memories"
                    );
                    return Ok(Some(v.to_string()));
                }
            }
        }
        Ok(None)
    }

    /// Retrieve a fact: **cache** → **MEMORY.md**. Does not query Codex; use [`Self::lookup_fact_by_key`] for DB fallback.
    #[deprecated(
        since = "0.3.0",
        note = "Direct explicit memory recall queries should transition to RAG. See Path C documentation."
    )]
    pub fn recall(&self, key: &str) -> Result<Option<String>, MemoryError> {
        tracing::warn!("Deprecated recall() called for key: {}", key);
        for fact in self.cache.iter().rev() {
            if fact.key == key {
                return Ok(Some(fact.value.clone()));
            }
        }
        self.long_term.get(key)
    }

    /// Cache → **MEMORY.md** → Codex `memories` (agent `global`, type `fact`).
    #[deprecated(
        since = "0.3.0",
        note = "Use `lookup_fact_by_key` instead; this alias remains for external callers."
    )]
    pub async fn recall_async(&self, key: &str) -> Result<Option<String>, MemoryError> {
        tracing::warn!("Deprecated recall_async() called for key: {}", key);
        self.lookup_fact_by_key(key).await
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
                let fact_line = format!("{key}: {value}");
                let fact_meta = format!("{{\"key\":\"{key}\"}}");
                let _ = db
                    .save_memory(vox_db::SaveMemoryParams {
                        agent_id: "global",
                        session_id: "sync",
                        memory_type: "fact",
                        content: &fact_line,
                        metadata: Some(fact_meta.as_str()),
                        importance: 1.0,
                        vcs_snapshot_id: None,
                    })
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

    /// Persist a stable campaign fact under the campaign namespace.
    pub fn persist_campaign_fact(
        &mut self,
        agent_id: AgentId,
        campaign_id: &str,
        fact: impl Into<String>,
    ) -> Result<(), MemoryError> {
        let fact = fact.into();
        let key = format!("campaign:{campaign_id}:fact:{}", self.cache.len());
        self.persist_fact(agent_id, key, fact, &[], None, None)
    }

    /// Persist a campaign hypothesis under the campaign namespace.
    pub fn persist_campaign_hypothesis(
        &mut self,
        agent_id: AgentId,
        campaign_id: &str,
        hypothesis: impl Into<String>,
    ) -> Result<(), MemoryError> {
        let hypothesis = hypothesis.into();
        let key = format!("campaign:{campaign_id}:hypothesis:{}", self.cache.len());
        self.persist_fact(agent_id, key, hypothesis, &[], None, None)
    }

    /// Persist a contradiction detected during a campaign.
    pub fn persist_campaign_contradiction(
        &mut self,
        agent_id: AgentId,
        campaign_id: &str,
        contradiction: impl Into<String>,
    ) -> Result<(), MemoryError> {
        let contradiction = contradiction.into();
        let key = format!("campaign:{campaign_id}:contradiction:{}", self.cache.len());
        self.persist_fact(agent_id, key, contradiction, &[], None, None)
    }

    /// Build a resumable campaign state from MEMORY.md namespaced keys.
    pub fn recall_campaign_snapshot(
        &self,
        campaign_id: &str,
    ) -> Result<crate::reconstruction::CampaignMemorySnapshot, MemoryError> {
        let mut snapshot = crate::reconstruction::CampaignMemorySnapshot {
            campaign_id: campaign_id.to_string(),
            ..Default::default()
        };
        for key in self.long_term.list_keys()? {
            let Some(value) = self.long_term.get(&key)? else {
                continue;
            };
            if !key.starts_with(&format!("campaign:{campaign_id}:")) {
                continue;
            }
            if key.contains(":fact:") {
                snapshot.stable_facts.push(value);
            } else if key.contains(":hypothesis:") {
                snapshot.hypotheses.push(value);
            } else if key.contains(":contradiction:") {
                snapshot.contradictions.push(value);
            } else if key.ends_with(":summary") {
                snapshot.milestone_summary = Some(value);
            }
        }
        Ok(snapshot)
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

            if let Ok(neighbors) = db.get_knowledge_neighbors(&node_id).await {
                for (target_id, target_label, _relation, _weight) in neighbors {
                    if visited.insert(target_id.clone()) {
                        let indent = "  ".repeat(current_depth + 1);
                        context.push_str(&format!("{indent}- {}\n", target_label));
                        queue.push_back((target_id, current_depth + 1));
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

        let log_name = format!("{}.md", today_str());
        let log_path = self.config.log_dir.join(&log_name);
        if let Ok(log_content) = vox_bounded_fs::read_utf8_path_capped(&log_path) {
            for (i, line) in log_content.lines().enumerate() {
                if line.to_lowercase().contains(&q) {
                    hits.push(SearchHit {
                        source: log_name.clone(),
                        line: i + 1,
                        content: line.to_string(),
                    });
                }
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

        // Temporal preamble
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        out.push_str(&format!(
            "Current date: {}.\nCurrent timestamp: {}s.\n\n",
            today_str(),
            secs
        ));

        // MEMORY.md
        if let Ok(mem) = self.long_term.read_all() {
            if !mem.trim().is_empty() {
                out.push_str("## Long-term Memory\n\n");
                out.push_str(&mem);
                out.push_str("\n\n");
            }
        }

        out
    }

    /// Like [`Self::bootstrap_context`], plus the workspace `VOX.md` project memory
    /// (when `workspace_root` is given and a project file exists). The project block is
    /// appended last so it carries the highest recency in the injected context.
    pub fn bootstrap_context_with_project(
        &self,
        workspace_root: Option<&std::path::Path>,
    ) -> String {
        let mut out = self.bootstrap_context();
        if let Some(block) = workspace_root.and_then(|root| self.project_context(root)) {
            out.push_str(&block);
            out.push_str("\n\n");
        }
        out
    }

    /// Load the workspace `VOX.md` project-memory block, or `None` when memory is
    /// disabled or no project file exists. See [`super::load_project_context`].
    pub fn project_context(&self, workspace_root: &std::path::Path) -> Option<String> {
        if !self.config.enabled {
            return None;
        }
        super::load_project_context(workspace_root)
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
            self.persist_fact(agent_id, &key, &value, &[], None, None)?;
            let _ = write!(summary, "{key}, ");
        }
        if count > 0 {
            let summary_str = format!(
                "[pre-compaction flush] Persisted {count} facts: {}",
                summary.trim_end_matches(", ")
            );
            let _ = self.log(&summary_str);
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
