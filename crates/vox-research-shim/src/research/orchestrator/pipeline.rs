use std::time::Instant;

use anyhow::Result;
use vox_db::Codex;
use vox_research_events::schema_types::FindingCandidateConfidence;
use vox_research_events::{
    DiscoverySignal, DiscoverySignalFamily, DiscoverySignalStrength, FindingCandidateClass,
    FindingCandidateV1, ResearchEvent, SignalProvenance,
};
use vox_search::{SearchPolicy, SearchRuntimeContext};

use super::super::claims::extract_claims_with_model;
use super::super::gate::{GateInput, score_with_config};
use super::super::planner::{decompose_query_with_config, plan_to_json};
use super::super::provider::ProviderRegistry;
use super::super::types::{
    Citation, CitationAuditResult, ClaimSupport, CompetenceSignal, ResearchHit, ResearchMetadata,
    ResearchPlan, ResearchQuery, ResearchResult, ResearchRunArtifact, ResearchScope, ResearchStage,
    RetrievalDiagnostics, RoutingTier,
};
use super::super::verifier::verify_claims_with_config;
use super::config::ResearchConfig;
use super::helpers::{fnv1a_hash, verifier_config_for_research_run};
use super::pipeline_cache::research_cache_short_circuit;
use super::stages::{
    JudgeParams, SynthesisParams, judge_quality, run_self_verification, synthesize_answer_with_llm,
};
use super::web_gather::{gather_local_hits_for_plan, gather_web_hits_for_plan};

/// Run the full research pipeline for `query`. Returns a `ResearchResult` with
/// a cited answer, sources, citations, and telemetry metadata.
pub async fn run_research(
    query: ResearchQuery,
    db: Option<&Codex>,
    config: &ResearchConfig,
) -> Result<ResearchResult> {
    run_research_with_context(query, None, db, config).await
}

/// Run research with an optional full retrieval context for local repo/memory fusion.
pub async fn run_research_with_context(
    query: ResearchQuery,
    search_ctx: Option<&SearchRuntimeContext>,
    db: Option<&Codex>,
    config: &ResearchConfig,
) -> Result<ResearchResult> {
    run_research_with_context_and_session(query, search_ctx, db, config, None).await
}

/// Run research using a pre-created session id, for async job submission paths.
pub async fn run_research_with_context_and_session(
    mut query: ResearchQuery,
    search_ctx: Option<&SearchRuntimeContext>,
    db: Option<&Codex>,
    config: &ResearchConfig,
    precreated_session_id: Option<i64>,
) -> Result<ResearchResult> {
    // ── (0) Check Rollout Flags ──────────────────────────────────────────────
    let web_enabled = true;
    let claim_enabled = config.claim_detection_enabled;
    // PHASE_0a_STUB: rollout flag queries not yet wired to Codex; always use defaults.
    // Phase 1 re-enables after vox_db gains get_retrieval_config.
    let persist_enabled = true;

    if let Some(db) = db
        && let Some(cached) = research_cache_short_circuit(&query, db, config).await
    {
        return Ok(cached);
    }

    if !web_enabled && query.scope == ResearchScope::Web {
        query.scope = ResearchScope::Both;
    }
    if !claim_enabled {
        query.verify_claims = false;
    }
    let _ = web_enabled; // used above

    let registry = ProviderRegistry::from_env_with_config(config.provider.clone());
    let llm_model_registry = vox_orchestrator::models::ModelRegistry::new();
    let base_inference = config.model_pick_inference.clone().unwrap_or_default();
    let resolved_llm =
        super::super::model_select::resolve_research_models(&llm_model_registry, &base_inference);
    let research_verifier_cfg = verifier_config_for_research_run(&config.verifier, &resolved_llm);
    tracing::info!(
        target: "vox_orchestrator::model_route",
        route_source = "research_pipeline",
        planner = %resolved_llm.planner_model,
        claim = %resolved_llm.claim_model,
        synthesis = %resolved_llm.synthesis_model,
        judge = %resolved_llm.judge_model,
        "research_models_resolved"
    );
    let start = Instant::now();

    let search_policy = resolved_search_policy_for_research_run(db, config).await;

    // ── (a) Session tracking ─────────────────────────────────────────────────
    let session_id: i64 = if let Some(id) = precreated_session_id {
        id
    } else if let Some(db) = db {
        let session_key = format!(
            "research:{:016x}",
            fnv1a_hash(&format!("{}|{:?}", query.query, query.scope))
        );
        db.create_research_session(&session_key, &query.query)
            .await
            .unwrap_or(0)
    } else {
        0
    };

    let report_progress = |msg: String, pct: Option<f32>| {
        if let Some(ref cb) = config.progress_callback {
            cb(msg, pct);
        }
    };
    emit_research_event(
        config,
        db,
        ResearchEvent::TelemetryObservation {
            provider: registry.primary_name().to_string(),
            metric_type: "research_started".to_string(),
            value: 1.0,
            session_id: session_id.to_string(),
            recorded_at_ms: now_ms_i64(),
        },
    );
    // Set status → planning before decomposition.
    set_session_stage(db, session_id, ResearchStage::Planning).await;

    report_progress(
        "Decomposing query into subqueries...".to_string(),
        Some(0.05),
    );

    // ── (b) Query decomposition ──────────────────────────────────────────────
    let plan: ResearchPlan = decompose_query_with_config(
        &query,
        config.llm_endpoint.as_deref(),
        config.api_key.as_deref(),
        Some(resolved_llm.planner_model.as_str()),
        Some(config.planner_temperature),
        Some(config.planner_max_subqueries),
    )
    .await
    .unwrap_or_else(|_| ResearchPlan {
        original_query: query.query.clone(),
        subqueries: vec![query.query.clone()],
        scope: query.scope.clone(),
        max_sources_per_subquery: query.max_sources,
    });
    emit_research_event(
        config,
        db,
        ResearchEvent::AggregateComputed {
            provider: registry.primary_name().to_string(),
            metric_type: "subqueries_emitted".to_string(),
            window_start_ms: now_ms_i64(),
            window_end_ms: now_ms_i64(),
            value: plan.subqueries.len() as f64,
            sample_count: plan.subqueries.len() as u64,
            session_id: session_id.to_string(),
        },
    );
    if let Some(db) = db
        && session_id > 0
    {
        let metadata_json =
            serde_json::to_string(&plan_to_json(&plan)).unwrap_or_else(|_| "{}".into());
        let _ = db
            .record_research_metric(
                session_id,
                "subqueries_emitted",
                plan.subqueries.len() as f64,
                Some(&metadata_json),
            )
            .await;
    }

    // Set status → retrieving before web/local gather.
    set_session_stage(db, session_id, ResearchStage::Retrieving).await;

    report_progress(
        format!("Running {} subqueries...", plan.subqueries.len()),
        Some(0.15),
    );

    let mut all_hits: Vec<ResearchHit> = Vec::new();
    let mut subqueries_with_hits = 0;
    let mut total_dropped_count = 0usize;
    let mut total_sources_attempted = 0usize;
    let do_local = matches!(query.scope, ResearchScope::Local | ResearchScope::Both);
    let do_web = matches!(query.scope, ResearchScope::Web | ResearchScope::Both);

    if do_local && let Some(ctx) = search_ctx {
        let (h, s, d, t) = gather_local_hits_for_plan(ctx, &query, &plan, &search_policy).await;
        subqueries_with_hits += s;
        total_dropped_count += d;
        total_sources_attempted += t;
        all_hits.extend(h);
    }

    if do_web {
        let (h, s, d, t) =
            gather_web_hits_for_plan(db, session_id, &query, &plan, &registry, &search_policy)
                .await;
        subqueries_with_hits += s;
        total_dropped_count += d;
        total_sources_attempted += t;
        all_hits.extend(h);
    }

    dedupe_hits_by_url(&mut all_hits);

    // ── (d) Retrieval diagnostics ─────────────────────────────────────────────
    let query_terms: Vec<&str> = query.query.split_whitespace().collect();
    let matched_terms = query_terms
        .iter()
        .filter(|term| {
            all_hits.iter().any(|h| {
                h.snippet
                    .to_ascii_lowercase()
                    .contains(&term.to_ascii_lowercase())
            })
        })
        .count();
    let coverage_pct = if query_terms.is_empty() {
        1.0
    } else {
        matched_terms as f64 / query_terms.len() as f64
    };
    let avg_score = if all_hits.is_empty() {
        0.0
    } else {
        all_hits.iter().map(|h| h.score).sum::<f64>() / all_hits.len() as f64
    };
    let subquery_coverage_pct = if plan.subqueries.is_empty() {
        0.0
    } else {
        subqueries_with_hits as f64 / plan.subqueries.len() as f64
    };
    let diagnostics = RetrievalDiagnostics {
        coverage_pct,
        subquery_coverage_pct,
        avg_provider_score: avg_score,
        fusion_weights: config.fusion_weights,
        dropped_source_count: total_dropped_count,
        hit_rate: if total_sources_attempted == 0 {
            0.0
        } else {
            (all_hits.len() as f64 / total_sources_attempted as f64).min(1.0)
        },
    };
    emit_research_event(
        config,
        db,
        ResearchEvent::AggregateComputed {
            provider: registry.primary_name().to_string(),
            metric_type: "retrieval_hit_rate".to_string(),
            window_start_ms: now_ms_i64(),
            window_end_ms: now_ms_i64(),
            value: diagnostics.hit_rate,
            sample_count: total_sources_attempted as u64,
            session_id: session_id.to_string(),
        },
    );
    emit_research_event(
        config,
        db,
        ResearchEvent::TelemetryObservation {
            provider: registry.primary_name().to_string(),
            metric_type: "sources_total".to_string(),
            value: all_hits.len() as f64,
            session_id: session_id.to_string(),
            recorded_at_ms: now_ms_i64(),
        },
    );

    // ── (e) Confidence gate → routing decision ────────────────────────────────
    let draft_claims = {
        let mut claims = extract_claims_with_model(
            &query.query,
            config.llm_endpoint.as_deref(),
            config.api_key.as_deref(),
            Some(resolved_llm.claim_model.as_str()),
            Some(config.claim_max_tokens),
        )
        .await;
        // rag-2: assign stable claim_id (FNV-1a hash of claim text) for ClaimCorrection references
        for claim in &mut claims {
            claim.claim_id = fnv1a_hash(&claim.text);
            emit_research_event(
                config,
                db,
                ResearchEvent::ClaimExtracted {
                    claim_id: claim.claim_id,
                    text: claim.text.clone(),
                    verifiability_score: 0.75,
                    session_id: session_id.to_string(),
                },
            );
        }
        claims
    };

    let gate_input = GateInput {
        claims: &draft_claims,
        citation_count: all_hits.len().min(3),
        no_retrieval_hits: all_hits.is_empty(),
        answer_is_empty: false,
    };
    let confidence_signal = score_with_config(&gate_input, &config.gate);
    let routing_tier = confidence_signal.routing_tier_for(
        config.routing_thresholds.direct,
        config.routing_thresholds.light,
        config.routing_thresholds.deep,
    );

    // ── (f) Claim verification ────────────────────────────────────────────────
    // Set status → verifying_claims before NLI classification.
    set_session_stage(db, session_id, ResearchStage::VerifyingClaims).await;
    report_progress("Verifying research claims...".to_string(), Some(0.60));
    let claim_verdicts = if query.verify_claims
        && matches!(routing_tier, RoutingTier::DeepResearch)
        && !draft_claims.is_empty()
    {
        verify_claims_with_config(
            &draft_claims,
            &query.query,
            &all_hits,
            &registry,
            &research_verifier_cfg,
            config.llm_endpoint.as_deref(),
            config.api_key.as_deref(),
        )
        .await
        // PHASE_0a_STUB: claim/verdict persistence not yet wired to vox_db.
        // Phase 1 re-enables after vox_db gains store_claim / store_claim_verdict / store_evidence_span.
    } else {
        vec![]
    };
    if let Some(db) = db
        && session_id > 0
    {
        for verdict in &claim_verdicts {
            let claim = &verdict.claim;
            let _ = db
                .store_claim(
                    session_id,
                    claim.claim_id,
                    &claim.text,
                    claim.is_numeric,
                    claim.is_recent,
                    claim.is_named_event,
                )
                .await;
            let _ = db
                .store_claim_verdict(
                    claim.claim_id,
                    &verdict.verdict.to_string(),
                    verdict.confidence,
                    &research_verifier_cfg.nli_model_id,
                )
                .await;
            for span in &verdict.evidence_spans {
                let _ = db
                    .store_evidence_span(claim.claim_id, span.span_start, span.span_end, &span.text)
                    .await;
            }
        }
    }
    for verdict in &claim_verdicts {
        if matches!(verdict.verdict, super::super::verifier::Verdict::Supported)
            && verdict.confidence > 0.8
        {
            emit_research_event(
                config,
                db,
                ResearchEvent::ClaimVerified {
                    claim_id: verdict.claim.claim_id,
                    verdict: verdict.verdict.to_string(),
                    confidence: verdict.confidence,
                    verifier_model: research_verifier_cfg.nli_model_id.clone(),
                    session_id: session_id.to_string(),
                },
            );
        }
    }

    // ── (h) Build citations ───────────────────────────────────────────────────
    let citations: Vec<Citation> = all_hits
        .iter()
        .take(10)
        .enumerate()
        .map(|(i, h)| Citation {
            source_id: i as i64,
            url: h.url.clone(),
            title: h.title.clone(),
            snippet: h.snippet.chars().take(300).collect(),
            confidence: h.score,
        })
        .collect();

    // ── (i) Synthesize answer ─────────────────────────────────────────────────
    // Set status → synthesizing before LLM synthesis call.
    set_session_stage(db, session_id, ResearchStage::Synthesizing).await;
    report_progress(
        "Synthesizing final research report...".to_string(),
        Some(0.85),
    );
    let answer = synthesize_answer_with_llm(SynthesisParams {
        query: &query.query,
        hits: &all_hits,
        verdicts: &claim_verdicts,
        endpoint: config.llm_endpoint.as_deref(),
        api_key: config.api_key.as_deref(),
        model: resolved_llm.synthesis_model.as_str(),
        temperature: config.synthesis_temperature,
        max_tokens: config.synthesis_max_tokens,
        context_max_chars: config.synthesis_context_max_chars,
    })
    .await;

    // ── (i) Evaluate final quality via judge ──────────────────────────────────
    let quality_score = judge_quality(JudgeParams {
        query: &query.query,
        answer: &answer,
        citations: &citations,
        endpoint: config.llm_endpoint.as_deref(),
        api_key: config.api_key.as_deref(),
        model: resolved_llm.judge_model.as_str(),
        temperature: config.judge_temperature,
        max_tokens: config.judge_max_tokens,
        fallback_score: config.fallback_quality_score,
    })
    .await;

    let self_verification_enabled = matches!(routing_tier, RoutingTier::DeepResearch);

    // PHASE_0a_STUB: training pair persistence not yet wired to vox_db.
    // Phase 1 re-enables after vox_db gains store_training_pair.

    // ── (j) Persistence ──────────────────────────────────────────────────────
    if persist_enabled
        && query.persist_to_docs
        && confidence_signal.score as f64 >= config.persist_min_confidence
        && answer.len() >= 200
    {
        use std::path::Path;
        if let Some(root_str) =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxWorkspaceRoot).expose()
        {
            let root = Path::new(root_str);
            let slug = super::super::persistence::slug_from_query(&query.query);
            let _ = super::super::persistence::write_research_doc(
                root,
                &slug,
                &query.query,
                &answer,
                resolved_llm.synthesis_model.as_str(),
            );
        }
    }

    // PHASE_0a_STUB: memory write not yet wired (save_memory API shape confirmed but
    // session management methods missing from vox_db).
    // Phase 1 re-enables after vox_db gains create_research_session etc.

    // ── (k) Citation audit stage ──────────────────────────────────────────────
    let duration_ms = start.elapsed().as_millis() as u64;
    // Set status → auditing_citations before citation audit computation.
    set_session_stage(db, session_id, ResearchStage::AuditingCitations).await;

    // ── (l) Optional CoVE-style self-verification ─────────────────────────────
    let self_verification =
        if self_verification_enabled && !answer.is_empty() && !all_hits.is_empty() {
            report_progress("Running self-verification step...".to_string(), Some(0.95));
            Some(
                run_self_verification(
                    &query.query,
                    &answer,
                    &all_hits,
                    config.llm_endpoint.as_deref(),
                    config.api_key.as_deref(),
                    resolved_llm.synthesis_model.as_str(),
                )
                .await,
            )
        } else {
            None
        };

    // ── (m) Compute CompetenceSignal ──────────────────────────────────────────
    let competence = Some(CompetenceSignal::from_verdicts(
        confidence_signal.score,
        quality_score,
        &claim_verdicts,
        // "verified" = we ran NLI classification on at least one claim
        !claim_verdicts.is_empty(),
    ));
    let supported_claim_ids: Vec<u64> = claim_verdicts
        .iter()
        .filter(|verdict| {
            matches!(verdict.verdict, super::super::verifier::Verdict::Supported)
                && verdict.confidence > 0.8
        })
        .map(|verdict| verdict.claim.claim_id)
        .collect();
    if quality_score >= 70 && supported_claim_ids.len() >= 3 {
        let finding_candidate = finding_candidate_from_research_run(
            session_id,
            &query,
            &answer,
            quality_score,
            &supported_claim_ids,
        );
        let finding_id = finding_candidate.candidate_id.clone();
        let _ = serde_json::to_value(&finding_candidate);
        emit_research_event(
            config,
            db,
            ResearchEvent::FindingCandidateProposed {
                finding_id,
                claim_ids: supported_claim_ids,
                worthiness_score: (quality_score as f64 / 100.0).clamp(0.0, 1.0),
                session_id: session_id.to_string(),
            },
        );
    }
    let citation_audit = audit_citations(&citations, &claim_verdicts);
    emit_research_event(
        config,
        db,
        ResearchEvent::AggregateComputed {
            provider: "vox-research".to_string(),
            metric_type: "citation_precision".to_string(),
            window_start_ms: now_ms_i64(),
            window_end_ms: now_ms_i64(),
            value: citation_audit.precision,
            sample_count: citation_audit.checked_citations as u64,
            session_id: session_id.to_string(),
        },
    );
    if let Some(self_verification) = &self_verification {
        emit_research_event(
            config,
            db,
            ResearchEvent::TelemetryObservation {
                provider: resolved_llm.synthesis_model.clone(),
                metric_type: "self_verification_reliability".to_string(),
                value: if self_verification.critical_inconsistency {
                    0.0
                } else {
                    1.0
                },
                session_id: session_id.to_string(),
                recorded_at_ms: now_ms_i64(),
            },
        );
    }

    let metadata = ResearchMetadata {
        session_id,
        duration_ms,
        provider: registry.primary_name().to_string(),
        routing_tier,
        confidence: confidence_signal.score as f64,
        subquery_count: plan.subqueries.len(),
        source_count: all_hits.len(),
        claim_verdicts: claim_verdicts.clone(),
        retrieval_diagnostics: diagnostics,
        quality_score,
        competence,
        self_verification,
        citation_audit: Some(citation_audit),
    };

    let result = ResearchResult {
        answer,
        sources: all_hits,
        citations,
        research_metadata: metadata,
    };

    // Set status → persisting_artifact before artifact store.
    set_session_stage(db, session_id, ResearchStage::PersistingArtifact).await;
    if let Some(db) = db
        && session_id > 0
    {
        let report_markdown = render_research_report_markdown(&query, &plan, &result);
        let artifact = ResearchRunArtifact {
            schema_version: 1,
            query: query.clone(),
            plan: plan.clone(),
            result: result.clone(),
            report_markdown: report_markdown.clone(),
        };
        if let Ok(artifact_json) = serde_json::to_string(&artifact) {
            let _ = db
                .store_research_artifact(session_id, &artifact_json, &report_markdown)
                .await;
        }
    }

    // Set status → completed after all work is done.
    set_session_stage(db, session_id, ResearchStage::Completed).await;

    Ok(result)
}

/// Best-effort stage status update. Errors are logged and swallowed so a
/// transient DB failure never aborts the research pipeline.
async fn set_session_stage(db: Option<&Codex>, session_id: i64, stage: ResearchStage) {
    if session_id <= 0 {
        return;
    }
    if let Some(db) = db
        && let Err(e) = db
            .update_research_session_status(session_id, stage.as_str())
            .await
    {
        tracing::warn!(
            session_id,
            stage = stage.as_str(),
            error = %e,
            "research_stage_update_failed"
        );
    }
}

fn dedupe_hits_by_url(hits: &mut Vec<ResearchHit>) {
    let mut seen = std::collections::HashSet::new();
    hits.retain(|hit| seen.insert(hit.url.clone()));
}

fn emit_research_event(config: &ResearchConfig, db: Option<&Codex>, event: ResearchEvent) {
    if let Some(emitter) = config.event_emitter.as_ref() {
        emitter.emit(event.clone());
    }
    if let Some(db) = db {
        crate::research::spawn_persist_research_event_for_metrics(db.clone(), event);
    }
}

async fn resolved_search_policy_for_research_run(
    db: Option<&Codex>,
    config: &ResearchConfig,
) -> SearchPolicy {
    let mut policy = SearchPolicy::from_env();
    let feedback = if let Some(fb) = config.search_policy_feedback {
        Some(fb)
    } else if let Some(db) = db {
        crate::research::load_rolling_search_policy_feedback(db).await
    } else {
        None
    };
    if let Some(fb) = feedback {
        policy = policy.with_scientia_feedback(fb);
    }
    policy
}

fn audit_citations(
    citations: &[Citation],
    verdicts: &[super::super::verifier::ClaimVerdict],
) -> CitationAuditResult {
    let mut unsupported_citation_indices = Vec::new();
    let mut supports = Vec::new();
    for (idx, citation) in citations.iter().enumerate() {
        let mut citation_supported = false;
        for verdict in verdicts {
            for span in &verdict.evidence_spans {
                if span.source_id == citation.source_id
                    && span.span_type == super::super::verifier::SpanType::Supporting
                    && quote_overlaps(&citation.snippet, &span.text)
                {
                    citation_supported = true;
                    supports.push(ClaimSupport {
                        claim_id: verdict.claim.claim_id,
                        citation_source_id: citation.source_id,
                        quote: span.text.clone(),
                        support_type: span.span_type,
                    });
                }
            }
        }
        if !citation_supported {
            unsupported_citation_indices.push(idx);
        }
    }
    let checked_citations = citations.len();
    let supported_citations = checked_citations - unsupported_citation_indices.len();
    CitationAuditResult {
        checked_citations,
        supported_citations,
        unsupported_citation_indices,
        precision: if checked_citations == 0 {
            1.0
        } else {
            supported_citations as f64 / checked_citations as f64
        },
        supports,
    }
}

fn quote_overlaps(citation_snippet: &str, quote: &str) -> bool {
    let snippet = citation_snippet.to_ascii_lowercase();
    let quote = quote.to_ascii_lowercase();
    if quote.len() >= 24 && snippet.contains(quote.trim()) {
        return true;
    }
    quote
        .split_whitespace()
        .filter(|token| token.len() >= 4)
        .filter(|token| snippet.contains(*token))
        .take(3)
        .count()
        >= 3
}

fn now_ms_i64() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

fn finding_candidate_from_research_run(
    session_id: i64,
    query: &ResearchQuery,
    answer: &str,
    quality_score: i32,
    supported_claim_ids: &[u64],
) -> FindingCandidateV1 {
    let now_ms = now_ms_i64();
    let signal = DiscoverySignal {
        code: "research_pipeline.supported_claims".to_string(),
        summary: format!(
            "{} supported claims from research run {}",
            supported_claim_ids.len(),
            session_id
        ),
        strength: DiscoverySignalStrength::Supporting,
        family: DiscoverySignalFamily::FindingCandidateSignal,
        source_ref: Some(format!("research-session:{session_id}")),
        provenance: SignalProvenance {
            origin: Some("vox-orchestrator.research_pipeline".to_string()),
            repo_path: None,
            metric_type: Some("supported_claims".to_string()),
            run_id: Some(session_id.to_string()),
            recorded_at_ms: Some(now_ms),
            digest: Some(format!("{:016x}", fnv1a_hash(answer))),
        },
    };
    FindingCandidateV1 {
        schema_version: 1,
        candidate_id: format!("finding-{session_id}-{:016x}", fnv1a_hash(answer)),
        candidate_class: FindingCandidateClass::Other,
        internal_signals: vec![signal],
        created_at_ms: now_ms,
        publication_id: None,
        title_hint: Some(query.query.chars().take(120).collect()),
        novelty_evidence_bundle_id: None,
        worthiness_decision_ref: Some(format!("research-quality-score:{quality_score}")),
        confidence: Some(FindingCandidateConfidence {
            signal_strength: Some((quality_score as f64 / 100.0).clamp(0.0, 1.0)),
            contradiction_risk: None,
            reproducibility_support: Some((supported_claim_ids.len() as f64 / 10.0).min(1.0)),
        }),
        updated_at_ms: None,
    }
}

fn render_research_report_markdown(
    query: &ResearchQuery,
    plan: &ResearchPlan,
    result: &ResearchResult,
) -> String {
    let mut out = String::new();
    out.push_str("# Research Report\n\n");
    out.push_str("## Query\n\n");
    out.push_str(&query.query);
    out.push_str("\n\n## Answer\n\n");
    out.push_str(result.answer.trim());
    out.push_str("\n\n## Research Plan\n\n");
    for subquery in &plan.subqueries {
        out.push_str("- ");
        out.push_str(subquery);
        out.push('\n');
    }
    out.push_str("\n## Sources\n\n");
    if result.sources.is_empty() {
        out.push_str("- No sources were retrieved.\n");
    } else {
        for (idx, source) in result.sources.iter().enumerate() {
            out.push_str(&format!(
                "- [{}] {} — {} (score {:.3})\n",
                idx + 1,
                source.title,
                source.url,
                source.score
            ));
        }
    }
    out.push_str("\n## Claim Ledger\n\n");
    if result.research_metadata.claim_verdicts.is_empty() {
        out.push_str("- No verified claims were produced for this run.\n");
    } else {
        for verdict in &result.research_metadata.claim_verdicts {
            out.push_str(&format!(
                "- {} — {} ({:.2})\n",
                verdict.claim.text, verdict.verdict, verdict.confidence
            ));
        }
    }
    out.push_str("\n## Citation Audit\n\n");
    if let Some(audit) = &result.research_metadata.citation_audit {
        out.push_str(&format!(
            "- Precision: {:.3}\n- Checked citations: {}\n- Supported citations: {}\n",
            audit.precision, audit.checked_citations, audit.supported_citations
        ));
        if !audit.unsupported_citation_indices.is_empty() {
            out.push_str(&format!(
                "- Unsupported citation indices: {:?}\n",
                audit.unsupported_citation_indices
            ));
        }
    } else {
        out.push_str("- Citation audit was not run.\n");
    }
    out.push_str("\n## Diagnostics\n\n");
    out.push_str(&format!(
        "- Routing tier: {:?}\n- Sources: {}\n- Quality score: {}\n",
        result.research_metadata.routing_tier,
        result.sources.len(),
        result.research_metadata.quality_score
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::super::super::verifier::{EvidenceSpan, SpanType, Verdict};
    use super::*;

    #[test]
    fn finding_candidate_matches_scientia_schema() {
        let query = ResearchQuery {
            query: "schema validation smoke".to_string(),
            scope: ResearchScope::Local,
            max_sources: 3,
            persist_to_docs: false,
            verify_claims: true,
            site_scope: None,
        };
        let candidate =
            finding_candidate_from_research_run(7, &query, "answer text", 84, &[1, 2, 3]);
        let schema: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../contracts/scientia/finding-candidate.v1.schema.json"
        )))
        .expect("schema parses");
        let value = serde_json::to_value(candidate).expect("candidate serializes");
        let validator = jsonschema::validator_for(&schema).expect("schema compiles");
        validator.validate(&value).expect("candidate validates");
    }

    #[test]
    fn research_run_artifact_matches_report_schema() {
        let query = ResearchQuery {
            query: "artifact schema smoke".to_string(),
            scope: ResearchScope::Local,
            max_sources: 3,
            persist_to_docs: false,
            verify_claims: false,
            site_scope: None,
        };
        let plan = ResearchPlan {
            original_query: query.query.clone(),
            subqueries: vec![query.query.clone()],
            scope: ResearchScope::Local,
            max_sources_per_subquery: 3,
        };
        let result = ResearchResult {
            answer: "A concise answer.".to_string(),
            sources: vec![],
            citations: vec![],
            research_metadata: ResearchMetadata {
                session_id: 1,
                duration_ms: 10,
                provider: "test".to_string(),
                routing_tier: RoutingTier::Direct,
                confidence: 0.5,
                subquery_count: 1,
                source_count: 0,
                claim_verdicts: vec![],
                retrieval_diagnostics: RetrievalDiagnostics::default(),
                quality_score: 50,
                competence: None,
                self_verification: None,
                citation_audit: None,
            },
        };
        let report_markdown = render_research_report_markdown(&query, &plan, &result);
        let artifact = ResearchRunArtifact {
            schema_version: 1,
            query,
            plan,
            result,
            report_markdown,
        };
        let schema: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../contracts/reports/research/artifact.v1.schema.json"
        )))
        .expect("schema parses");
        let value = serde_json::to_value(artifact).expect("artifact serializes");
        let validator = jsonschema::validator_for(&schema).expect("schema compiles");
        validator.validate(&value).expect("artifact validates");
    }

    #[test]
    fn citation_audit_requires_quote_backed_support() {
        let citations = vec![Citation {
            source_id: 0,
            url: "repo://doc".to_string(),
            title: "Doc".to_string(),
            snippet: "The system supports durable research artifacts.".to_string(),
            confidence: 0.9,
        }];
        let verdicts = vec![super::super::super::verifier::ClaimVerdict {
            claim: super::super::super::claims::Claim {
                claim_id: 1,
                text: "The system supports durable research artifacts.".to_string(),
                is_numeric: false,
                is_recent: false,
                is_named_event: false,
            },
            verdict: Verdict::Supported,
            confidence: 0.9,
            supporting_count: 1,
            contradicting_count: 0,
            evidence_spans: vec![EvidenceSpan {
                source_id: 0,
                span_start: 4,
                span_end: 41,
                text: "supports durable research artifacts".to_string(),
                span_type: SpanType::Supporting,
            }],
        }];

        let audit = audit_citations(&citations, &verdicts);

        assert_eq!(audit.checked_citations, 1);
        assert_eq!(audit.supported_citations, 1);
        assert!(audit.unsupported_citation_indices.is_empty());
        assert_eq!(audit.precision, 1.0);
    }
}
