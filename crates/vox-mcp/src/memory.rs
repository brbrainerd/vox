//! MCP tools for the persistent memory system.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{ServerState, ToolResult};

// ---------------------------------------------------------------------------
// Parameters
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MemoryStoreParams {
    pub agent_id: u64,
    pub key: String,
    pub value: String,
    pub relations: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KnowledgeQueryParams {
    pub query: String,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MemoryRecallParams {
    pub key: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MemorySearchParams {
    pub query: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MemoryLogParams {
    pub entry: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CompactParams {
    pub agent_id: u64,
    pub summary: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SessionCreateParams {
    pub agent_id: u64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SessionIdParams {
    pub session_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SessionCompactParams {
    pub session_id: String,
    pub summary: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SessionAddTurnParams {
    pub session_id: String,
    pub role: String,
    pub content: String,
}

// ---------------------------------------------------------------------------
// Memory tool handlers
// ---------------------------------------------------------------------------

/// Persist a key-value fact to long-term memory (MEMORY.md + VoxDb).
pub async fn memory_store(state: &ServerState, params: MemoryStoreParams) -> String {
    let config = vox_orchestrator::MemoryConfig::default();
    match vox_orchestrator::MemoryManager::new(config) {
        Ok(mut mgr) => {
            if let Some(ref db) = state.db {
                mgr.set_db(db.clone());
            }
            let rels = params.relations.unwrap_or_default();
            let rel_strs: Vec<&str> = rels.iter().map(|s| s.as_str()).collect();
            match mgr.persist_fact(
                vox_orchestrator::AgentId(params.agent_id),
                &params.key,
                &params.value,
                &rel_strs,
            ) {
                Ok(()) => ToolResult::ok(format!("Stored '{}' = '{}'", params.key, params.value))
                    .to_json(),
                Err(e) => ToolResult::<String>::err(format!("{e}")).to_json(),
            }
        }
        Err(e) => ToolResult::<String>::err(format!("memory init failed: {e}")).to_json(),
    }
}

/// Retrieve a fact from long-term memory by key.
pub async fn memory_recall(_state: &ServerState, params: MemoryRecallParams) -> String {
    let config = vox_orchestrator::MemoryConfig::default();
    match vox_orchestrator::MemoryManager::new(config) {
        Ok(mgr) => match mgr.recall(&params.key) {
            Ok(Some(val)) => ToolResult::ok(val).to_json(),
            Ok(None) => {
                ToolResult::<String>::err(format!("Key '{}' not found", params.key)).to_json()
            }
            Err(e) => ToolResult::<String>::err(format!("{e}")).to_json(),
        },
        Err(e) => ToolResult::<String>::err(format!("memory init failed: {e}")).to_json(),
    }
}

/// Search memory (daily logs + MEMORY.md) by keyword.
pub async fn memory_search(_state: &ServerState, params: MemorySearchParams) -> String {
    let config = vox_orchestrator::MemoryConfig::default();
    match vox_orchestrator::MemoryManager::new(config) {
        Ok(mgr) => match mgr.search(&params.query) {
            Ok(hits) => {
                if hits.is_empty() {
                    ToolResult::ok("No results found.".to_string()).to_json()
                } else {
                    let formatted = hits
                        .iter()
                        .map(|h| format!("[{}:{}] {}", h.source, h.line, h.content))
                        .collect::<Vec<_>>()
                        .join("\n");
                    ToolResult::ok(formatted).to_json()
                }
            }
            Err(e) => ToolResult::<String>::err(format!("{e}")).to_json(),
        },
        Err(e) => ToolResult::<String>::err(format!("memory init failed: {e}")).to_json(),
    }
}

/// Append an entry to today's daily memory log.
pub async fn memory_daily_log(_state: &ServerState, params: MemoryLogParams) -> String {
    let config = vox_orchestrator::MemoryConfig::default();
    match vox_orchestrator::MemoryManager::new(config) {
        Ok(mgr) => match mgr.log(&params.entry) {
            Ok(()) => ToolResult::ok("Entry logged to daily memory.".to_string()).to_json(),
            Err(e) => ToolResult::<String>::err(format!("{e}")).to_json(),
        },
        Err(e) => ToolResult::<String>::err(format!("memory init failed: {e}")).to_json(),
    }
}

/// List all memory keys from MEMORY.md.
pub async fn memory_list_keys(_state: &ServerState) -> String {
    let config = vox_orchestrator::MemoryConfig::default();
    match vox_orchestrator::MemoryManager::new(config) {
        Ok(mgr) => match mgr.list_keys() {
            Ok(keys) => ToolResult::ok(keys).to_json(),
            Err(e) => ToolResult::<String>::err(format!("{e}")).to_json(),
        },
        Err(e) => ToolResult::<String>::err(format!("memory init failed: {e}")).to_json(),
    }
}

/// Query the knowledge graph by keyword.
pub async fn knowledge_query(state: &ServerState, params: KnowledgeQueryParams) -> String {
    if let Some(ref db) = state.db {
        let limit = params.limit.unwrap_or(10);
        match db.store().query_knowledge_nodes(&params.query, limit).await {
            Ok(nodes) => {
                if nodes.is_empty() {
                    ToolResult::ok("No related knowledge nodes found.".to_string()).to_json()
                } else {
                    let formatted = nodes
                        .into_iter()
                        .map(|(id, ntype, label)| format!("[{}] {} ({})", id, label, ntype))
                        .collect::<Vec<_>>()
                        .join("\n");
                    ToolResult::ok(formatted).to_json()
                }
            }
            Err(e) => ToolResult::<String>::err(format!("{e}")).to_json(),
        }
    } else {
        ToolResult::<String>::err("VoxDb not attached to MCP server.".to_string()).to_json()
    }
}

// ---------------------------------------------------------------------------
// Compaction tool handlers
// ---------------------------------------------------------------------------

/// Get current context window usage and compaction recommendation (async).
pub async fn compaction_status(
    state: &ServerState,
    params: crate::context::ContextBudgetParams,
) -> String {
    let g = state.orchestrator.lock().await;
    let orch = g;
    let id = vox_orchestrator::AgentId(params.agent_id);
    if let Some(budget) = orch.budget().check_budget(id) {
        let engine = vox_orchestrator::CompactionEngine::default();
        let should = engine.should_compact(budget.tokens_used);
        ToolResult::ok(format!(
            "Agent {}: {}/{} tokens used. Compaction recommended: {}. Strategy: {}",
            params.agent_id,
            budget.tokens_used,
            budget.model_max_tokens,
            should,
            vox_orchestrator::CompactionStrategy::default()
        ))
        .to_json()
    } else {
        ToolResult::ok(format!(
            "Agent {}: no budget tracked. Compaction engine ready with {}k token limit.",
            params.agent_id,
            vox_orchestrator::CompactionConfig::default().max_context_tokens / 1000
        ))
        .to_json()
    }
}

// ---------------------------------------------------------------------------
// Session tool handlers
// ---------------------------------------------------------------------------

/// Response type for session info.
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub agent_id: u64,
    pub state: String,
    pub turn_count: usize,
    pub total_tokens: usize,
    pub last_active: u64,
}

/// Create a new session for an agent (async).
pub async fn session_create(state: &ServerState, params: SessionCreateParams) -> String {
    let mut mgr = state.session_manager.lock().await;
    match mgr.create(vox_orchestrator::AgentId(params.agent_id)) {
        Ok(id) => ToolResult::ok(id).to_json(),
        Err(e) => ToolResult::<String>::err(format!("{e}")).to_json(),
    }
}

/// List all sessions (async).
pub async fn session_list(state: &ServerState) -> String {
    let mgr = state.session_manager.lock().await;
    let sessions: Vec<SessionInfo> = mgr
        .list_sessions()
        .iter()
        .map(|s| SessionInfo {
            id: s.id.clone(),
            agent_id: s.agent_id.0,
            state: s.state.to_string(),
            turn_count: s.turn_count,
            total_tokens: s.total_tokens,
            last_active: s.last_active,
        })
        .collect();
    ToolResult::ok(sessions).to_json()
}

/// Reset a session (clear history, keep metadata) (async).
pub async fn session_reset(state: &ServerState, params: SessionIdParams) -> String {
    let mut mgr = state.session_manager.lock().await;
    match mgr.reset(&params.session_id) {
        Ok(cleared) => ToolResult::ok(format!(
            "Session '{}' reset: {} turns cleared.",
            params.session_id, cleared
        ))
        .to_json(),
        Err(e) => ToolResult::<String>::err(format!("{e}")).to_json(),
    }
}

/// Compact a session with a summary (async).
pub async fn session_compact(state: &ServerState, params: SessionCompactParams) -> String {
    let mut mgr = state.session_manager.lock().await;
    match mgr.compact(&params.session_id, &params.summary) {
        Ok(removed) => ToolResult::ok(format!(
            "Session '{}' compacted: {} turns replaced with summary.",
            params.session_id, removed
        ))
        .to_json(),
        Err(e) => ToolResult::<String>::err(format!("{e}")).to_json(),
    }
}

/// Get info about a specific session (async).
pub async fn session_info(state: &ServerState, params: SessionIdParams) -> String {
    let mgr = state.session_manager.lock().await;
    match mgr.get(&params.session_id) {
        Some(s) => ToolResult::ok(SessionInfo {
            id: s.id.clone(),
            agent_id: s.agent_id.0,
            state: s.state.to_string(),
            turn_count: s.turn_count,
            total_tokens: s.total_tokens,
            last_active: s.last_active,
        })
        .to_json(),
        None => ToolResult::<String>::err(format!("Session '{}' not found.", params.session_id))
            .to_json(),
    }
}

/// Cleanup archived sessions (async).
pub async fn session_cleanup(state: &ServerState) -> String {
    let mut mgr = state.session_manager.lock().await;
    mgr.tick_lifecycle();
    match mgr.cleanup() {
        Ok(n) => ToolResult::ok(format!("{n} sessions cleaned up.")).to_json(),
        Err(e) => ToolResult::<String>::err(format!("{e}")).to_json(),
    }
}

// ---------------------------------------------------------------------------
// Preference & Behavioral Learning tool handlers
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PreferenceGetParams {
    pub user_id: String,
    pub key: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PreferenceSetParams {
    pub user_id: String,
    pub key: String,
    pub value: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PreferenceListParams {
    pub user_id: String,
    pub prefix: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LearnPatternParams {
    pub user_id: String,
    pub pattern_type: String,
    pub category: String,
    pub description: String,
    pub confidence: Option<f64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BehaviorRecordParams {
    pub user_id: String,
    pub event_type: String,
    pub context: Option<String>,
    pub metadata: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BehaviorSummaryParams {
    pub user_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MemorySaveDbParams {
    pub agent_id: String,
    pub session_id: String,
    pub memory_type: String,
    pub content: String,
    pub importance: Option<f64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MemoryRecallDbParams {
    pub agent_id: String,
    pub memory_type: Option<String>,
    pub limit: Option<i64>,
}

/// Get a user preference from VoxDb.
pub async fn preference_get(state: &ServerState, params: PreferenceGetParams) -> String {
    match &state.db {
        None => ToolResult::<String>::err("VoxDb not attached").to_json(),
        Some(db) => match db
            .store()
            .get_user_preference(&params.user_id, &params.key)
            .await
        {
            Ok(Some(val)) => ToolResult::ok(val).to_json(),
            Ok(None) => ToolResult::<String>::err(format!("Preference '{}' not found", params.key))
                .to_json(),
            Err(e) => ToolResult::<String>::err(format!("{e}")).to_json(),
        },
    }
}

/// Set a user preference in VoxDb.
pub async fn preference_set(state: &ServerState, params: PreferenceSetParams) -> String {
    match &state.db {
        None => ToolResult::<String>::err("VoxDb not attached").to_json(),
        Some(db) => match db
            .store()
            .set_user_preference(&params.user_id, &params.key, &params.value)
            .await
        {
            Ok(()) => {
                ToolResult::ok(format!("Set '{}' = '{}'", params.key, params.value)).to_json()
            }
            Err(e) => ToolResult::<String>::err(format!("{e}")).to_json(),
        },
    }
}

/// List user preferences from VoxDb, optionally filtered by key prefix.
pub async fn preference_list(state: &ServerState, params: PreferenceListParams) -> String {
    match &state.db {
        None => ToolResult::<String>::err("VoxDb not attached").to_json(),
        Some(db) => match db.store().list_user_preferences(&params.user_id).await {
            Ok(prefs) => {
                let filtered: Vec<(String, String)> = if let Some(prefix) = params.prefix {
                    prefs
                        .into_iter()
                        .filter(|(k, _)| k.starts_with(&prefix))
                        .collect()
                } else {
                    prefs
                };
                let lines: Vec<String> =
                    filtered.iter().map(|(k, v)| format!("{k} = {v}")).collect();
                ToolResult::ok(lines.join("\n")).to_json()
            }
            Err(e) => ToolResult::<String>::err(format!("{e}")).to_json(),
        },
    }
}

/// Store a learned behavior pattern in VoxDb.
pub async fn learn_pattern(state: &ServerState, params: LearnPatternParams) -> String {
    match &state.db {
        None => ToolResult::<String>::err("VoxDb not attached").to_json(),
        Some(db) => match db
            .store()
            .store_learned_pattern(
                &params.user_id,
                &params.pattern_type,
                &params.category,
                &params.description,
                params.confidence.unwrap_or(0.5),
            )
            .await
        {
            Ok(id) => ToolResult::ok(format!("Pattern stored with id={id}")).to_json(),
            Err(e) => ToolResult::<String>::err(format!("{e}")).to_json(),
        },
    }
}

/// Record a user behavior event and get triggered suggestions.
pub async fn behavior_record(state: &ServerState, params: BehaviorRecordParams) -> String {
    match &state.db {
        None => ToolResult::<String>::err("VoxDb not attached").to_json(),
        Some(db) => {
            let learner = db.learner();
            match learner
                .observe(
                    &params.user_id,
                    &params.event_type,
                    params.context.as_deref(),
                    params.metadata.as_deref(),
                )
                .await
            {
                Ok(suggestions) => {
                    if suggestions.is_empty() {
                        ToolResult::ok("Event recorded. No new patterns detected.".to_string())
                            .to_json()
                    } else {
                        let lines: Vec<String> = suggestions
                            .iter()
                            .map(|s| {
                                format!(
                                    "[{:.0}%] {}: {}",
                                    s.confidence * 100.0,
                                    s.title,
                                    s.description
                                )
                            })
                            .collect();
                        ToolResult::ok(format!(
                            "Event recorded. New patterns:\n{}",
                            lines.join("\n")
                        ))
                        .to_json()
                    }
                }
                Err(e) => ToolResult::<String>::err(format!("{e}")).to_json(),
            }
        }
    }
}

/// Analyze all behavior events for a user and return learned patterns summary.
pub async fn behavior_summary(state: &ServerState, params: BehaviorSummaryParams) -> String {
    match &state.db {
        None => ToolResult::<String>::err("VoxDb not attached").to_json(),
        Some(db) => {
            let learner = db.learner();
            match learner.analyze(&params.user_id).await {
                Ok(patterns) => {
                    if patterns.is_empty() {
                        ToolResult::ok("No patterns detected yet.".to_string()).to_json()
                    } else {
                        let lines: Vec<String> = patterns
                            .iter()
                            .map(|p| {
                                format!(
                                    "[{:.0}%] {} / {} — {}",
                                    p.confidence * 100.0,
                                    p.pattern_type,
                                    p.category,
                                    p.description
                                )
                            })
                            .collect();
                        ToolResult::ok(lines.join("\n")).to_json()
                    }
                }
                Err(e) => ToolResult::<String>::err(format!("{e}")).to_json(),
            }
        }
    }
}

/// Persist a fact directly into VoxDb agent_memory table.
pub async fn memory_save_db(state: &ServerState, params: MemorySaveDbParams) -> String {
    match &state.db {
        None => ToolResult::<String>::err("VoxDb not attached").to_json(),
        Some(db) => match db
            .store()
            .save_memory(
                &params.agent_id,
                &params.session_id,
                &params.memory_type,
                &params.content,
                None,
                params.importance.unwrap_or(1.0),
                None,
            )
            .await
        {
            Ok(id) => ToolResult::ok(format!("Memory saved with id={id}")).to_json(),
            Err(e) => ToolResult::<String>::err(format!("{e}")).to_json(),
        },
    }
}

/// Recall facts from VoxDb agent_memory table.
pub async fn memory_recall_db(state: &ServerState, params: MemoryRecallDbParams) -> String {
    match &state.db {
        None => ToolResult::<String>::err("VoxDb not attached").to_json(),
        Some(db) => match db
            .store()
            .recall_memory(
                &params.agent_id,
                params.memory_type.as_deref(),
                params.limit.unwrap_or(20),
                None,
            )
            .await
        {
            Ok(entries) => {
                if entries.is_empty() {
                    ToolResult::ok("No memories found.".to_string()).to_json()
                } else {
                    let lines: Vec<String> = entries
                        .iter()
                        .map(|e| format!("[{}] [{:.2}] {}", e.memory_type, e.importance, e.content))
                        .collect();
                    ToolResult::ok(lines.join("\n")).to_json()
                }
            }
            Err(e) => ToolResult::<String>::err(format!("{e}")).to_json(),
        },
    }
}
