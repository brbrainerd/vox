use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use vox_search::{
    LexicalMemoryFallback, RetrievalTriggerMode as BundleTrigger, SearchPolicy,
    SearchRuntimeContext, diagnostics_value, run_search_with_verification, search_plan_value,
};

use crate::server_state::ServerState;

/// Why retrieval is being invoked for this turn/tool path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalTriggerMode {
    /// Silent preamble enrichment for chat turns.
    AutoChatPreamble,
    /// Explicit user call through a retrieval/search tool.
    ExplicitToolQuery,
    /// Additional retrieval pass used for contradiction/risk verification.
    VerificationPass,
}

/// Structured retrieval metadata shared between MCP surfaces and Socrates telemetry.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RetrievalEvidenceEnvelope {
    /// Trigger mode that initiated this retrieval pass.
    pub trigger: RetrievalTriggerMode,
    /// Effective execution tier: `hybrid`, `bm25`, `lexical_fallback`, or `none`.
    pub retrieval_tier: String,
    /// Number of memory hits returned by the selected retrieval tier.
    pub memory_hit_count: usize,
    /// Number of knowledge graph rows returned from VoxDb.
    pub knowledge_hit_count: usize,
    /// Ingested `search_document_chunks` hits (RAG corpus).
    #[serde(default)]
    pub chunk_hit_count: usize,
    /// Repository path or inventory hits returned for repo-oriented queries.
    #[serde(default)]
    pub repo_hit_count: usize,
    /// Whether the vector leg contributed evidence.
    pub used_vector: bool,
    /// Whether BM25/keyword ranking contributed evidence.
    pub used_bm25: bool,
    /// Whether lexical fallback (substring scan) was used.
    pub used_lexical_fallback: bool,
    /// Contradiction hints detected in merged retrieval output.
    pub contradiction_count: usize,
    /// Highest fused score in returned memory hits.
    pub top_score: Option<f64>,
    /// Planner-selected intent for this search.
    #[serde(default)]
    pub search_intent: String,
    /// Planner-selected retrieval mode.
    #[serde(default)]
    pub selected_mode: String,
    /// Backends that contributed evidence during execution.
    #[serde(default)]
    pub backend_mix: Vec<String>,
    /// Distinct corpora that returned evidence.
    #[serde(default)]
    pub source_diversity: usize,
    /// Coarse evidence quality estimate in `[0, 1]`.
    #[serde(default)]
    pub evidence_quality: f64,
    /// Citation coverage proxy in `[0, 1]`.
    #[serde(default)]
    pub citation_coverage: f64,
    /// Whether an automatic verification/refinement pass was executed.
    #[serde(default)]
    pub verification_performed: bool,
    /// Why a verification/refinement pass ran.
    #[serde(default)]
    pub verification_reason: Option<String>,
    /// Query used for the verification/refinement pass.
    #[serde(default)]
    pub verification_query: Option<String>,
    /// Recommended next action for Socrates / callers when evidence is still weak.
    #[serde(default)]
    pub recommended_next_action: Option<String>,
    /// Planner JSON preserved for telemetry/debugging.
    #[serde(default)]
    pub search_plan: Value,
    /// Execution diagnostics preserved for telemetry/debugging.
    #[serde(default)]
    pub search_diagnostics: Value,
    /// Observed `PRAGMA journal_mode` when a DB was available (telemetry / routing hints).
    #[serde(default)]
    pub sqlite_journal_mode: Option<String>,
    /// Whether compile options suggested FTS5 (see [`vox_db::capabilities::SqliteProbeSnapshot`]).
    #[serde(default)]
    pub sqlite_fts5_reported: Option<bool>,
    /// Whether `PRAGMA foreign_keys` reported enforcement.
    #[serde(default)]
    pub sqlite_foreign_keys_on: Option<bool>,
    /// Cross-corpus RRF fused hits (non-zero when policy `prefer_rrf_merge` and ≥2 corpora).
    #[serde(default)]
    pub rrf_fused_hit_count: usize,
}

impl RetrievalEvidenceEnvelope {
    /// Convert retrieval evidence into canonical context envelope shape for cross-surface handoff.
    #[must_use]
    pub fn to_context_envelope(
        &self,
        repository_id: &str,
        session_id: Option<&str>,
    ) -> vox_orchestrator::ContextEnvelope {
        let now = vox_orchestrator::now_unix_ms();
        let sid = session_id.map(std::string::ToString::to_string);
        let hit_count = self
            .memory_hit_count
            .saturating_add(self.knowledge_hit_count)
            .saturating_add(self.chunk_hit_count)
            .saturating_add(self.repo_hit_count)
            .saturating_add(self.rrf_fused_hit_count);
        let contradiction_ratio = if hit_count == 0 {
            None
        } else {
            Some((self.contradiction_count as f64 / hit_count as f64).clamp(0.0, 1.0))
        };
        vox_orchestrator::ContextEnvelope {
            schema_version: 1,
            envelope_type: vox_orchestrator::ContextEnvelopeType::RetrievalEvidence,
            envelope_id: format!(
                "mcp-retrieval-{}-{now}",
                sid.as_deref().unwrap_or("no-session")
            ),
            created_at_unix_ms: now,
            expires_at_unix_ms: Some(now.saturating_add(3_600_000)),
            ttl_seconds: Some(3600),
            provenance: vox_orchestrator::ContextProvenance {
                source_plane: vox_orchestrator::ContextSourcePlane::Mcp,
                source_system: "vox-mcp".to_string(),
                source_tool: Some("retrieval_bundle".to_string()),
                source_path: None,
                producer_agent_id: Some("0".to_string()),
                producer_node_id: None,
                producer_session_id: sid.clone(),
                producer_thread_id: None,
                capture_mode: vox_orchestrator::ContextCaptureMode::Retrieved,
                policy_version: None,
                observed_via: vec!["run_retrieval_bundle".to_string()],
                trace_id: None,
                correlation_id: None,
            },
            trust: vox_orchestrator::ContextTrust {
                trust_tier: vox_orchestrator::ContextTrustTier::Trusted,
                authority_rank: 70,
                freshness_tier: vox_orchestrator::ContextFreshnessTier::Recent,
                confidence: Some(self.evidence_quality.clamp(0.0, 1.0)),
                contradiction_ratio,
                requires_citation: Some(hit_count == 0),
                may_override_lower_authority: Some(false),
            },
            lineage: None,
            subject: vox_orchestrator::ContextSubject {
                repository_id: repository_id.to_string(),
                workspace_id: None,
                session_id: sid,
                thread_id: None,
                task_id: None,
                goal_id: None,
                agent_id: Some("0".to_string()),
                receiver_agent_id: None,
                node_id: None,
                populi_scope_id: None,
                surface: Some("retrieval".to_string()),
            },
            content: vox_orchestrator::ContextContent {
                summary_text: format!(
                    "tier={} quality={:.3} contradiction_count={} verification_performed={}",
                    self.retrieval_tier,
                    self.evidence_quality,
                    self.contradiction_count,
                    self.verification_performed
                ),
                facts: Vec::new(),
                repo_paths: Vec::new(),
                artifact_refs: Vec::new(),
                citations: Vec::new(),
                tags: vec![
                    "retrieval".to_string(),
                    self.retrieval_tier.clone(),
                    self.selected_mode.clone(),
                ],
                structured_payload: serde_json::to_value(self).ok(),
                truncated_warnings: Vec::new(),
            },
            conflict_policy: vox_orchestrator::ContextConflictPolicy {
                merge_strategy: vox_orchestrator::ContextMergeStrategy::ConfidenceWeighted,
                stale_after_ms: Some(3_600_000),
                dedupe_key: Some(format!(
                    "retrieval:{repository_id}:{}",
                    session_id.unwrap_or("none")
                )),
                overwrite_requires_evidence: Some(true),
                conflict_class: Some(vox_orchestrator::ContextConflictClass::SourceTrust),
            },
            budget: vox_orchestrator::ContextBudget {
                priority: vox_orchestrator::ContextPriority::Normal,
                injection_mode: vox_orchestrator::ContextInjectionMode::Inline,
                token_estimate: None,
                max_tokens_for_injection: None,
                retrieval_cost_class: Some(vox_orchestrator::ContextRetrievalCostClass::Moderate),
                must_refresh_before_use: Some(false),
            },
            safety: Some(vox_orchestrator::ContextSafety {
                risk_budget: Some("normal".to_string()),
                factual_mode: Some(true),
                required_citations: Some(if hit_count == 0 { 1 } else { 0 }),
            }),
            obo_token: None,
            operating_mode: None,
            agentos: None,
        }
    }
}

/// Internal retrieval payload used by chat preamble and memory tools.
#[derive(Debug, Clone)]
pub struct RetrievalBundle {
    pub memory_lines: Vec<String>,
    pub knowledge_lines: Vec<String>,
    pub chunk_lines: Vec<String>,
    pub repo_lines: Vec<String>,
    /// RRF-merged excerpt ordering (same lines as corpora, deduped; empty when disabled).
    pub rrf_fused_lines: Vec<String>,
    pub evidence: RetrievalEvidenceEnvelope,
}

struct McpMemoryFallback {
    cfg: vox_orchestrator::MemoryConfig,
}

impl LexicalMemoryFallback for McpMemoryFallback {
    fn substring_search_lines(&self, query: &str) -> Result<Vec<String>, String> {
        let mgr =
            vox_orchestrator::MemoryManager::new(self.cfg.clone()).map_err(|e| e.to_string())?;
        let hits = mgr.search(query).map_err(|e| e.to_string())?;
        Ok(hits
            .into_iter()
            .map(|h| format!("[{}:{}] {}", h.source, h.line, h.content))
            .collect())
    }
}

fn map_trigger(t: RetrievalTriggerMode) -> BundleTrigger {
    match t {
        RetrievalTriggerMode::AutoChatPreamble => BundleTrigger::AutoChatPreamble,
        RetrievalTriggerMode::ExplicitToolQuery => BundleTrigger::ExplicitToolQuery,
        RetrievalTriggerMode::VerificationPass => BundleTrigger::VerificationPass,
    }
}

fn format_backend_mix(backends: &[vox_db::SearchBackend]) -> Vec<String> {
    backends
        .iter()
        .map(|b| format!("{b:?}").to_ascii_lowercase())
        .collect()
}

/// Unified retrieval trigger used by chat preamble + explicit search tools.
pub async fn run_retrieval_bundle(
    state: &ServerState,
    query: &str,
    trigger: RetrievalTriggerMode,
    limit: usize,
    trace_id: Option<&str>,
) -> Result<RetrievalBundle, String> {
    let sqlite_cap = match (&state.sqlite_capabilities, state.db.as_ref()) {
        (Some(s), _) => Some(s.clone()),
        (None, Some(db)) => db.sqlite_capabilities_snapshot().await.ok(),
        _ => None,
    };

    let policy = SearchPolicy::from_env();
    let trace = trace_id
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string);
    let ctx = SearchRuntimeContext::new(
        state.repository.root.clone(),
        state.db.clone(),
        state.orchestrator_config.memory.log_dir.clone(),
        state.orchestrator_config.memory.memory_md_path.clone(),
    )
    .with_trace_id(trace);
    let fallback: Option<Box<dyn LexicalMemoryFallback>> =
        if state.orchestrator_config.memory.enabled {
            Some(Box::new(McpMemoryFallback {
                cfg: state.orchestrator_config.memory.clone(),
            }))
        } else {
            None
        };
    let lex = fallback.as_deref();

    let (execution, diagnostics, plan) =
        run_search_with_verification(&ctx, query, map_trigger(trigger), limit, &policy, lex, None)
            .await?;

    let mut chunk_lines = execution.chunk_lines.clone();
    chunk_lines.extend_from_slice(&execution.tantivy_doc_lines);
    chunk_lines.extend_from_slice(&execution.qdrant_lines);

    let retrieval_tier = if execution.used_vector && execution.used_bm25 {
        "hybrid"
    } else if execution.used_bm25 {
        "bm25"
    } else if execution.lexical_fallback_used {
        "lexical_fallback"
    } else {
        "none"
    };

    let (sqlite_journal_mode, sqlite_fts5_reported, sqlite_foreign_keys_on) =
        sqlite_cap.map_or((None, None, None), |p| {
            (
                Some(p.journal_mode.clone()),
                Some(p.fts5_reported),
                Some(p.foreign_keys_on),
            )
        });

    Ok(RetrievalBundle {
        evidence: RetrievalEvidenceEnvelope {
            trigger,
            retrieval_tier: retrieval_tier.to_string(),
            memory_hit_count: execution.memory_lines.len(),
            knowledge_hit_count: execution.knowledge_lines.len(),
            chunk_hit_count: chunk_lines.len(),
            repo_hit_count: execution.repo_lines.len(),
            used_vector: execution.used_vector,
            used_bm25: execution.used_bm25,
            used_lexical_fallback: execution.lexical_fallback_used,
            contradiction_count: execution.contradiction_count,
            top_score: execution.top_score,
            search_intent: format!("{:?}", plan.intent).to_ascii_lowercase(),
            selected_mode: format!("{:?}", plan.retrieval_mode).to_ascii_lowercase(),
            backend_mix: format_backend_mix(&execution.backend_mix),
            source_diversity: execution.source_diversity,
            evidence_quality: execution.evidence_quality,
            citation_coverage: execution.citation_coverage,
            verification_performed: diagnostics.verification_performed,
            verification_reason: diagnostics.verification_reason.clone(),
            verification_query: diagnostics.verification_query.clone(),
            recommended_next_action: diagnostics
                .recommended_action
                .or(execution.recommended_next_action)
                .map(|a| format!("{a:?}").to_ascii_lowercase()),
            search_plan: search_plan_value(&plan),
            search_diagnostics: diagnostics_value(&diagnostics),
            sqlite_journal_mode,
            sqlite_fts5_reported,
            sqlite_foreign_keys_on,
            rrf_fused_hit_count: execution.rrf_fused_lines.len(),
        },
        memory_lines: execution.memory_lines,
        knowledge_lines: execution.knowledge_lines,
        chunk_lines,
        repo_lines: execution.repo_lines,
        rrf_fused_lines: execution.rrf_fused_lines,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn retrieval_bundle_respects_verification_trigger_mode() {
        let state = crate::ServerState::new_test().await;
        let bundle = run_retrieval_bundle(
            &state,
            "verify contradictory evidence for retrieval",
            RetrievalTriggerMode::VerificationPass,
            3,
            Some("test-trace-retrieval"),
        )
        .await
        .expect("retrieval bundle");
        assert!(
            !bundle.evidence.selected_mode.is_empty(),
            "selected mode should be populated"
        );
        assert!(matches!(
            bundle.evidence.trigger,
            RetrievalTriggerMode::VerificationPass
        ));
        assert_eq!(bundle.evidence.search_intent, "verification");
        assert_eq!(bundle.evidence.selected_mode, "hybrid");
        assert!(!bundle.evidence.retrieval_tier.is_empty());
    }
}
