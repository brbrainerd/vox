use super::config::memory_config_for_state;
use super::params::{
    KnowledgeQueryParams, MemoryLogParams, MemoryRecallParams, MemorySearchParams,
    MemoryStoreParams, ResearchRunParams, ResearchSessionParams, ResearchStartParams,
    SemanticFsDiscoverParams,
};
use super::retrieval::{RetrievalTriggerMode, run_retrieval_bundle};
use crate::params::ToolResult;
use crate::server_state::ServerState;

use serde_json::json;
use vox_search::SearchPolicy;

const REM_MEMORY_VOXDB: &str =
    "Attach VoxDb (`VOX_DB_PATH` / `VOX_DB_URL`) to the MCP server for knowledge-graph queries.";
const REM_MEMORY_INIT: &str = "Verify orchestrator memory paths and permissions; restart the MCP server if config is inconsistent.";
const REM_MEMORY_PERSIST: &str =
    "Check disk quotas and MEMORY.md / daily log paths; ensure the agent id is valid.";
const REM_MEMORY_KEY: &str =
    "Store the fact first or run `memory_list_keys`; keys are case-sensitive.";
const REM_MEMORY_RETRIEVAL: &str =
    "Verify corpus/index paths and RAG settings; see orchestrator memory configuration.";
const REM_MEMORY_KG_QUERY: &str =
    "Check Turso connectivity, vox-db migrations, and that the knowledge graph tables exist.";
const REM_RESEARCH_RUN: &str = "Configure web backends (`VOX_SEARCH_SEARXNG_URL`, DuckDuckGo fallback, optional Tavily per Tavily SSOT). For synthesis/judge, set LLM endpoint env used by orchestrator.";

/// Persist a key-value fact to long-term memory (MEMORY.md + VoxDb).
pub async fn memory_store(state: &ServerState, params: MemoryStoreParams) -> String {
    let config = memory_config_for_state(state);
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
                params.media_url.as_deref(),
                params.media_type.as_deref(),
            ) {
                Ok(()) => ToolResult::ok(format!("Stored '{}' = '{}'", params.key, params.value))
                    .to_json(),
                Err(e) => {
                    ToolResult::<String>::err_with_remediation(format!("{e}"), REM_MEMORY_PERSIST)
                        .to_json()
                }
            }
        }
        Err(e) => ToolResult::<String>::err_with_remediation(
            format!("memory init failed: {e}"),
            REM_MEMORY_INIT,
        )
        .to_json(),
    }
}

/// Retrieve a fact from long-term memory by key.
///
/// Uses the same `MemoryConfig` as `memory_store` (`state.orchestrator_config.memory`).
/// When `state.db` is set, recall includes Codex `memories` after file miss (`MemoryManager::lookup_fact_by_key`).
pub async fn memory_recall(state: &ServerState, params: MemoryRecallParams) -> String {
    let config = memory_config_for_state(state);
    match vox_orchestrator::MemoryManager::new(config) {
        Ok(mut mgr) => {
            if let Some(ref db) = state.db {
                mgr.set_db(db.clone());
            }
            match mgr.lookup_fact_by_key(&params.key).await {
                Ok(Some(val)) => ToolResult::ok(val).to_json(),
                Ok(None) => ToolResult::<String>::err_with_remediation(
                    format!("Key '{}' not found", params.key),
                    REM_MEMORY_KEY,
                )
                .to_json(),
                Err(e) => {
                    ToolResult::<String>::err_with_remediation(format!("{e}"), REM_MEMORY_PERSIST)
                        .to_json()
                }
            }
        }
        Err(e) => ToolResult::<String>::err_with_remediation(
            format!("memory init failed: {e}"),
            REM_MEMORY_INIT,
        )
        .to_json(),
    }
}

/// Search memory (daily logs + MEMORY.md) by keyword.
pub async fn memory_search(state: &ServerState, params: MemorySearchParams) -> String {
    let trace = params
        .trace_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            params
                .correlation_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
        });
    match run_retrieval_bundle(
        state,
        &params.query,
        RetrievalTriggerMode::ExplicitToolQuery,
        10,
        trace,
    )
    .await
    {
        Ok(bundle) => {
            if bundle.memory_lines.is_empty()
                && bundle.knowledge_lines.is_empty()
                && bundle.chunk_lines.is_empty()
                && bundle.repo_lines.is_empty()
                && bundle.rrf_fused_lines.is_empty()
            {
                ToolResult::ok("No results found.".to_string()).to_json()
            } else {
                let mut out = Vec::new();
                out.push(format!(
                    "retrieval_tier={} trigger={:?} used_vector={} used_bm25={} lexical_fallback={} contradictions={} knowledge_hits={} chunk_hits={} repo_hits={} rrf_fused_hits={} mode={} intent={} verification_performed={} recommended_next_action={}",
                    bundle.evidence.retrieval_tier,
                    bundle.evidence.trigger,
                    bundle.evidence.used_vector,
                    bundle.evidence.used_bm25,
                    bundle.evidence.used_lexical_fallback,
                    bundle.evidence.contradiction_count,
                    bundle.evidence.knowledge_hit_count,
                    bundle.evidence.chunk_hit_count,
                    bundle.evidence.repo_hit_count,
                    bundle.evidence.rrf_fused_hit_count,
                    bundle.evidence.selected_mode,
                    bundle.evidence.search_intent,
                    bundle.evidence.verification_performed,
                    bundle
                        .evidence
                        .recommended_next_action
                        .as_deref()
                        .unwrap_or("none"),
                ));
                if !bundle.rrf_fused_lines.is_empty() {
                    out.push("[FUSED_RRF]".to_string());
                    out.extend(bundle.rrf_fused_lines.clone());
                }
                if !bundle.memory_lines.is_empty() {
                    out.push("[MEMORY]".to_string());
                    out.extend(bundle.memory_lines);
                }
                if !bundle.knowledge_lines.is_empty() {
                    out.push("[KNOWLEDGE_GRAPH]".to_string());
                    out.extend(bundle.knowledge_lines);
                }
                if !bundle.chunk_lines.is_empty() {
                    out.push("[DOCUMENT_CHUNKS]".to_string());
                    out.extend(bundle.chunk_lines);
                }
                if !bundle.repo_lines.is_empty() {
                    out.push("[REPO]".to_string());
                    out.extend(bundle.repo_lines);
                }
                ToolResult::ok(out.join("\n")).to_json()
            }
        }
        Err(e) => ToolResult::<String>::err_with_remediation(e, REM_MEMORY_RETRIEVAL).to_json(),
    }
}

/// Rank workspace paths for an intent via the semantic-fs bridge (read-only inventory search).
pub async fn semantic_fs_discover_mcp(
    state: &ServerState,
    params: SemanticFsDiscoverParams,
) -> String {
    let policy = SearchPolicy::from_env();
    let lim_raw = params.limit.unwrap_or(16);
    let limit = lim_raw.clamp(1, 256) as usize;
    let hits =
        super::semantic_fs_discover(&state.repository.root, params.intent.trim(), limit, &policy);
    ToolResult::ok(json!({ "hits": hits })).to_json()
}

/// Append an entry to today's daily memory log.
///
/// Uses the same orchestrator-scoped memory paths as other memory tools.
pub async fn memory_daily_log(state: &ServerState, params: MemoryLogParams) -> String {
    let config = memory_config_for_state(state);
    match vox_orchestrator::MemoryManager::new(config) {
        Ok(mut mgr) => {
            if let Some(ref db) = state.db {
                mgr.set_db(db.clone());
            }
            match mgr.log(&params.entry) {
                Ok(()) => ToolResult::ok("Entry logged to daily memory.".to_string()).to_json(),
                Err(e) => {
                    ToolResult::<String>::err_with_remediation(format!("{e}"), REM_MEMORY_PERSIST)
                        .to_json()
                }
            }
        }
        Err(e) => ToolResult::<String>::err_with_remediation(
            format!("memory init failed: {e}"),
            REM_MEMORY_INIT,
        )
        .to_json(),
    }
}

/// List all memory keys from MEMORY.md.
pub async fn memory_list_keys(state: &ServerState) -> String {
    let config = memory_config_for_state(state);
    match vox_orchestrator::MemoryManager::new(config) {
        Ok(mgr) => match mgr.list_keys() {
            Ok(keys) => ToolResult::ok(keys).to_json(),
            Err(e) => {
                ToolResult::<String>::err_with_remediation(format!("{e}"), REM_MEMORY_PERSIST)
                    .to_json()
            }
        },
        Err(e) => ToolResult::<String>::err_with_remediation(
            format!("memory init failed: {e}"),
            REM_MEMORY_INIT,
        )
        .to_json(),
    }
}

/// Query the knowledge graph by keyword.
pub async fn knowledge_query(state: &ServerState, params: KnowledgeQueryParams) -> String {
    if let Some(ref db) = state.db {
        let limit = params.limit.unwrap_or(10);
        match db.query_knowledge_nodes(&params.query, limit).await {
            Ok(nodes) => {
                if nodes.is_empty() {
                    ToolResult::ok("No related knowledge nodes found.".to_string()).to_json()
                } else {
                    let formatted = nodes
                        .into_iter()
                        .map(|(id, label, snippet)| {
                            format!("[{}] {} — {}", id, label, snippet.replace('\n', " "))
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    ToolResult::ok(formatted).to_json()
                }
            }
            Err(e) => {
                ToolResult::<String>::err_with_remediation(format!("{e}"), REM_MEMORY_KG_QUERY)
                    .to_json()
            }
        }
    } else {
        ToolResult::<String>::err_with_remediation(
            "VoxDb not attached to MCP server.".to_string(),
            REM_MEMORY_VOXDB,
        )
        .to_json()
    }
}

/// Run the orchestrator research pipeline (`run_research`): web gather via `vox-search`, synthesis, judge.
pub async fn research_run(state: &ServerState, params: ResearchRunParams) -> String {
    use vox_dei_shim::research::{
        ResearchConfig, ResearchQuery, ResearchScope, run_research_with_context,
    };
    use vox_search::SearchRuntimeContext;

    let scope_label = params.scope.as_deref().map(str::trim).unwrap_or("both");
    let scope = match scope_label.to_ascii_lowercase().as_str() {
        "" | "both" => ResearchScope::Both,
        "local" => ResearchScope::Local,
        "web" => ResearchScope::Web,
        other => {
            return ToolResult::<String>::err_with_remediation(
                format!("invalid scope {other:?}: use web|local|both"),
                REM_RESEARCH_RUN,
            )
            .to_json();
        }
    };

    let rq = ResearchQuery {
        query: params.query,
        scope,
        max_sources: params.max_sources.unwrap_or(10).clamp(1, 50),
        persist_to_docs: false,
        verify_claims: params.verify_claims.unwrap_or(false),
        site_scope: params.site_scope,
    };

    let config = ResearchConfig {
        event_emitter: Some(std::sync::Arc::new(
            vox_dei_shim::research::BroadcastEmitter::new(state.research_events.clone()),
        )),
        ..ResearchConfig::default()
    };
    let ctx = SearchRuntimeContext::new(
        state.repository.root.clone(),
        state.db.clone(),
        state.orchestrator_config.memory.log_dir.clone(),
        state.orchestrator_config.memory.memory_md_path.clone(),
    );

    match run_research_with_context(rq, Some(&ctx), state.db.as_deref(), &config).await {
        Ok(result) => {
            if params.json {
                match serde_json::to_string(&result) {
                    Ok(s) => ToolResult::ok(s).to_json(),
                    Err(e) => ToolResult::<String>::err_with_remediation(
                        format!("serialize failed: {e}"),
                        REM_RESEARCH_RUN,
                    )
                    .to_json(),
                }
            } else {
                let mut out = String::new();
                out.push_str(&result.answer);
                out.push_str("\n\n## Sources\n");
                for h in &result.sources {
                    out.push_str(&format!("- {} — {}\n", h.title, h.url));
                }
                ToolResult::ok(out).to_json()
            }
        }
        Err(e) => {
            ToolResult::<String>::err_with_remediation(format!("{e}"), REM_RESEARCH_RUN).to_json()
        }
    }
}

pub async fn research_start(state: &ServerState, params: ResearchStartParams) -> String {
    use vox_dei_shim::research::{
        ResearchConfig, ResearchQuery, run_research_with_context_and_session,
    };
    use vox_search::SearchRuntimeContext;

    let Some(db) = state.db.clone() else {
        return ToolResult::<String>::err_with_remediation(
            "VoxDb not attached to MCP server.".to_string(),
            REM_MEMORY_VOXDB,
        )
        .to_json();
    };
    let scope = match parse_research_scope(params.scope.as_deref()) {
        Ok(scope) => scope,
        Err(msg) => {
            return ToolResult::<String>::err_with_remediation(msg, REM_RESEARCH_RUN).to_json();
        }
    };
    let query = params.query.trim().to_string();
    if query.is_empty() {
        return ToolResult::<String>::err_with_remediation(
            "query must not be empty".to_string(),
            REM_RESEARCH_RUN,
        )
        .to_json();
    }
    let session_key = format!("research_async:{}", uuid::Uuid::new_v4());
    let session_id = match db.create_research_session(&session_key, &query).await {
        Ok(id) => id,
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(format!("{e}"), REM_RESEARCH_RUN)
                .to_json();
        }
    };
    let _ = db
        .update_research_session_status(session_id, "running")
        .await;

    let state = state.clone();
    tokio::spawn(async move {
        let rq = ResearchQuery {
            query,
            scope,
            max_sources: params.max_sources.unwrap_or(10).clamp(1, 50),
            persist_to_docs: false,
            verify_claims: params.verify_claims.unwrap_or(false),
            site_scope: params.site_scope,
        };
        let ctx = SearchRuntimeContext::new(
            state.repository.root.clone(),
            state.db.clone(),
            state.orchestrator_config.memory.log_dir.clone(),
            state.orchestrator_config.memory.memory_md_path.clone(),
        );
        let config = ResearchConfig {
            event_emitter: Some(std::sync::Arc::new(
                vox_dei_shim::research::BroadcastEmitter::new(state.research_events.clone()),
            )),
            ..ResearchConfig::default()
        };
        let outcome = run_research_with_context_and_session(
            rq,
            Some(&ctx),
            state.db.as_deref(),
            &config,
            Some(session_id),
        )
        .await;
        if let Some(db) = state.db.as_ref() {
            if outcome.is_err() {
                let _ = db
                    .update_research_session_status(session_id, "failed")
                    .await;
            }
        }
    });

    ToolResult::ok(
        serde_json::json!({
            "session_id": session_id,
            "task_id": format!("research-{session_id}"),
            "status": "running"
        })
        .to_string(),
    )
    .to_json()
}

pub async fn research_status(state: &ServerState, params: ResearchSessionParams) -> String {
    let Some(db) = state.db.as_ref() else {
        return ToolResult::<String>::err_with_remediation(
            "VoxDb not attached to MCP server.".to_string(),
            REM_MEMORY_VOXDB,
        )
        .to_json();
    };
    match db.get_research_session(params.session_id).await {
        Ok(Some(row)) => {
            let percent_complete = match row.status.as_str() {
                "completed" => 1.0,
                "failed" => 1.0,
                "running" | "active" => 0.5,
                _ => 0.1,
            };
            ToolResult::ok(
                serde_json::json!({
                    "session_id": row.id,
                    "stage": row.status,
                    "percent_complete": percent_complete,
                    "query": row.query_text,
                    "last_event_ts": row.finished_at_ms.unwrap_or(row.started_at_ms)
                })
                .to_string(),
            )
            .to_json()
        }
        Ok(None) => ToolResult::<String>::err_with_remediation(
            format!("research session {} not found", params.session_id),
            REM_RESEARCH_RUN,
        )
        .to_json(),
        Err(e) => {
            ToolResult::<String>::err_with_remediation(format!("{e}"), REM_RESEARCH_RUN).to_json()
        }
    }
}

pub async fn research_get(state: &ServerState, params: ResearchSessionParams) -> String {
    let Some(db) = state.db.as_ref() else {
        return ToolResult::<String>::err_with_remediation(
            "VoxDb not attached to MCP server.".to_string(),
            REM_MEMORY_VOXDB,
        )
        .to_json();
    };
    let session = match db.get_research_session(params.session_id).await {
        Ok(Some(row)) => row,
        Ok(None) => {
            return ToolResult::<String>::err_with_remediation(
                format!("research session {} not found", params.session_id),
                REM_RESEARCH_RUN,
            )
            .to_json();
        }
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(format!("{e}"), REM_RESEARCH_RUN)
                .to_json();
        }
    };
    let artifact = match db.get_research_artifact(params.session_id).await {
        Ok(artifact) => artifact,
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(format!("{e}"), REM_RESEARCH_RUN)
                .to_json();
        }
    };
    ToolResult::ok(
        serde_json::json!({
            "session": session,
            "artifact": artifact,
        })
        .to_string(),
    )
    .to_json()
}

fn parse_research_scope(
    scope: Option<&str>,
) -> Result<vox_dei_shim::research::ResearchScope, String> {
    use vox_dei_shim::research::ResearchScope;
    match scope
        .map(str::trim)
        .unwrap_or("both")
        .to_ascii_lowercase()
        .as_str()
    {
        "" | "both" => Ok(ResearchScope::Both),
        "local" => Ok(ResearchScope::Local),
        "web" => Ok(ResearchScope::Web),
        other => Err(format!("invalid scope {other:?}: use web|local|both")),
    }
}
