//! Verification pass orchestration (shared between MCP and orchestrator).

use serde_json::Value;

use vox_db::{RetrievalMode, SearchDiagnostics, SearchIntent, SearchPlan, heuristic_search_plan};

use crate::context::SearchRuntimeContext;
use crate::execution::{
    LexicalMemoryFallback, SearchExecution, best_effort_verification_query, execute_search_plan,
};
use crate::policy::SearchPolicy;

/// Trigger mode parity with MCP (kept numeric-free here; MCP maps to its enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetrievalTriggerMode {
    AutoChatPreamble,
    ExplicitToolQuery,
    VerificationPass,
}

/// Run planner + optional second pass using policy thresholds.
pub async fn run_search_with_verification(
    ctx: &SearchRuntimeContext,
    query: &str,
    trigger: RetrievalTriggerMode,
    limit: usize,
    policy: &SearchPolicy,
    lexical_fallback: Option<&dyn LexicalMemoryFallback>,
    budget: Option<&crate::tavily_budget::TavilySessionBudget>,
) -> Result<(SearchExecution, SearchDiagnostics, SearchPlan), String> {
    let plan = heuristic_search_plan(
        query,
        trigger == RetrievalTriggerMode::VerificationPass,
        None,
    );
    let mut execution =
        execute_search_plan(ctx, query, &plan, limit, policy, lexical_fallback).await?;
    let corpora_line = plan
        .corpora
        .iter()
        .map(|c| format!("{c:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    let selection_rationale = vec![
        format!(
            "intent={}",
            format!("{:?}", plan.intent).to_ascii_lowercase()
        ),
        format!(
            "mode={}",
            format!("{:?}", plan.retrieval_mode).to_ascii_lowercase()
        ),
        format!("corpora=[{corpora_line}]"),
    ];
    let mut diagnostics = SearchDiagnostics {
        policy_version: policy.version,
        selection_rationale,
        selected_mode: Some(plan.retrieval_mode),
        backends_used: execution.backend_mix.clone(),
        evidence_quality: execution.evidence_quality,
        citation_coverage: execution.citation_coverage,
        source_diversity: execution.source_diversity,
        initial_top_score: execution.top_score,
        recommended_action: execution.recommended_next_action,
        notes: plan.notes.clone(),
        ..SearchDiagnostics::default()
    };
    diagnostics
        .notes
        .push(format!("search_policy_version={}", policy.version));
    for w in &execution.warnings {
        if !w.is_empty() {
            diagnostics.notes.push(w.clone());
        }
    }
    if !execution.rrf_fused_lines.is_empty() {
        diagnostics
            .notes
            .push("rrf_fusion=active (see rrf_fused_lines in execution / MCP bundle)".to_string());
    }

    let threshold = policy.verification_weak_evidence_threshold;
    let should_verify = trigger != RetrievalTriggerMode::VerificationPass
        && plan.allow_verification_pass
        && (execution.contradiction_count > 0
            || execution.source_diversity <= 1
            || execution.evidence_quality < threshold
            || execution.lexical_fallback_used
            || (execution.memory_lines.is_empty()
                && execution.knowledge_lines.is_empty()
                && execution.chunk_lines.is_empty()
                && execution.repo_lines.is_empty()
                && execution.tantivy_doc_lines.is_empty()
                && execution.qdrant_lines.is_empty()
                && execution.rrf_fused_lines.is_empty()));
    if should_verify
        && let Some(verification_query) = best_effort_verification_query(query)
        && verification_query != plan.normalized_query
    {
        let mut verification_plan = plan.clone();
        verification_plan.intent = SearchIntent::Verification;
        verification_plan.retrieval_mode = RetrievalMode::Hybrid;
        verification_plan.allow_verification_pass = false;
        verification_plan.rewritten_query = Some(verification_query.clone());
        let verified = execute_search_plan(
            ctx,
            &verification_query,
            &verification_plan,
            limit,
            policy,
            lexical_fallback,
        )
        .await?;
        diagnostics.verification_performed = true;
        diagnostics.verification_reason = Some(if execution.contradiction_count > 0 {
            "contradictions_detected".to_string()
        } else if execution.lexical_fallback_used {
            "lexical_fallback_only".to_string()
        } else if execution.source_diversity <= 1 {
            "single_corpus_evidence".to_string()
        } else {
            "weak_evidence_quality".to_string()
        });
        diagnostics.verification_query = Some(verification_query.clone());
        diagnostics.verified_top_score = verified.top_score;
        diagnostics.verification_top_score_delta = match (verified.top_score, execution.top_score) {
            (Some(after), Some(before)) => Some(after - before),
            _ => None,
        };
        if verified.evidence_quality > execution.evidence_quality
            || verified.source_diversity > execution.source_diversity
        {
            execution = verified;
        }
    }

    #[cfg(feature = "tavily")]
    {
        let is_empty = execution.memory_lines.is_empty()
            && execution.knowledge_lines.is_empty()
            && execution.chunk_lines.is_empty()
            && execution.repo_lines.is_empty()
            && execution.tantivy_doc_lines.is_empty()
            && execution.qdrant_lines.is_empty();

        if policy.tavily_enabled {
            let tavily_should_fire = (policy.tavily_fire_on_empty && is_empty)
                || (policy.tavily_fire_on_weak && execution.evidence_quality < threshold);

            if tavily_should_fire {
                if let Some(b) = budget
                    && !b.try_consume(1)
                {
                    execution
                        .warnings
                        .push("tavily_budget_exhausted".to_string());
                    return Ok((execution, diagnostics, plan));
                }
                if let Some(client) = crate::tavily::TavilySearchClient::from_env() {
                    match client
                        .search(
                            query,
                            policy.tavily_max_results,
                            &policy.tavily_search_depth,
                        )
                        .await
                    {
                        Ok(hits) => {
                            if let Some(b) = budget {
                                let rem = b.remaining();
                                diagnostics
                                    .notes
                                    .push(format!("tavily_credits_remaining={rem}"));
                                if rem <= 20 {
                                    execution
                                        .warnings
                                        .push("Tavily session budget >=80% exhausted".to_string());
                                }
                            }
                            let mut t_lines = Vec::new();
                            for h in &hits {
                                t_lines.push(format!(
                                    "[crag_tavily:{}] {} (score: {:.3})",
                                    h.url,
                                    h.content.replace('\n', " "),
                                    h.score
                                ));

                                if let Some(db) = &ctx.db {
                                    let url = h.url.clone();
                                    let content = h.content.clone();
                                    let title = h.title.clone();
                                    let db_arc = db.clone();
                                    tokio::spawn(async move {
                                        let source_uri = format!("tavily:{}", url);
                                        let _ = crate::ingest::persist_text_document_chunk(
                                            db_arc.as_ref(),
                                            &source_uri,
                                            &title,
                                            &content,
                                            "text/plain",
                                        )
                                        .await;
                                    });
                                }
                            }
                            diagnostics
                                .notes
                                .push("crag_tavily_triggered=true".to_string());
                            diagnostics
                                .notes
                                .push(format!("tavily_results_count={}", t_lines.len()));

                            for h in &hits {
                                let mut hasher = blake3::Hasher::new();
                                hasher.update(b"a2a-search-token");
                                hasher.update(h.url.as_bytes());
                                let token = format!("a2a_{}", hasher.finalize().to_hex());
                                execution.durable_artifacts.push(
                                    crate::execution::DurableArtifact {
                                        uri: format!("tavily:{}", h.url),
                                        token: Some(token),
                                        expires_at_unix_ms: None,
                                        chunk_count: 1,
                                    },
                                );
                            }

                            execution.web_lines.extend(t_lines.iter().cloned());
                            if policy.prefer_rrf_merge {
                                execution.rrf_fused_lines.extend(t_lines);
                            }
                        }
                        Err(e) => {
                            execution.warnings.push(e);
                        }
                    }
                } else {
                    execution
                        .warnings
                        .push("tavily_triggered_but_missing_auth".to_string());
                }
            }
        }
    }

    Ok((execution, diagnostics, plan))
}

/// JSON snapshots for telemetry fields carried by MCP envelopes.
pub fn search_plan_value(plan: &SearchPlan) -> Value {
    serde_json::to_value(plan).unwrap_or(Value::Null)
}

pub fn diagnostics_value(diag: &SearchDiagnostics) -> Value {
    serde_json::to_value(diag).unwrap_or(Value::Null)
}
