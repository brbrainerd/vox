use super::*;
use crate::params::SubmitTaskParams;

fn base_params(description: &str) -> SubmitTaskParams {
    SubmitTaskParams {
        description: description.to_string(),
        files: vec![],
        priority: None,
        agent_name: None,
        capabilities: None,
        task_category: None,
        complexity: None,
        model_preference: None,
        model_override: None,
        session_id: None,
        thread_id: None,
        planning_mode: None,
        goal_type: None,
        retrieval: None,
        context_envelope_json: None,
        harness_spec_json: None,
        goal_scope: None,
        max_plan_depth: None,
        campaign_id: None,
        benchmark_tier: None,
        trace_id: None,
        correlation_id: None,
        tool_hints: vec![],
        research_hints: vec![],
        required_labels: None,
        is_detached: None,
        budget: None,
        active_skill: None,
        tenant_id: None,
        clutch: None,
        risk: None,
        trigger_source: None,
        chat_session_id: None,
    }
}

#[test]
fn parse_campaign_from_description_extracts_campaign_and_tier_tokens() {
    let (cid, tier) =
        submission::parse_campaign_from_description("do work [campaign:alpha1] [tier:crate_regen]");
    assert_eq!(cid.as_deref(), Some("alpha1"));
    assert_eq!(
        tier,
        Some(vox_orchestrator::ReconstructionBenchmarkTier::CrateRegen)
    );
}

#[test]
fn parse_campaign_from_description_is_case_insensitive_for_prefixes() {
    let (cid, tier) =
        submission::parse_campaign_from_description("do work [Campaign:Alpha] [TIER:repo_regen]");
    assert_eq!(cid.as_deref(), Some("Alpha"));
    assert_eq!(
        tier,
        Some(vox_orchestrator::ReconstructionBenchmarkTier::RepoRegen)
    );
}

#[test]
fn enqueue_hints_from_submit_params_returns_none_when_no_signals_present() {
    let params = base_params("plain task");
    assert!(submission::enqueue_hints_from_submit_params(&params).is_none());
}

#[test]
fn enqueue_hints_from_submit_params_maps_testing_category_to_verifier_role() {
    let mut params = base_params("run tests");
    params.task_category = Some("testing".to_string());
    let hints = submission::enqueue_hints_from_submit_params(&params).expect("hints");
    assert_eq!(
        hints.execution_role,
        Some(vox_orchestrator::AgentExecutionRole::Verifier)
    );
}

#[test]
fn enqueue_hints_from_submit_params_merges_campaign_tokens_from_description() {
    let mut params = base_params("fix bug campaign:campA tier:issue_repair");
    params.complexity = Some(8);
    let hints = submission::enqueue_hints_from_submit_params(&params).expect("hints");
    assert_eq!(hints.campaign_id.as_deref(), Some("campA"));
    assert_eq!(
        hints.benchmark_tier,
        Some(vox_orchestrator::ReconstructionBenchmarkTier::IssueRepair)
    );
    assert_eq!(hints.complexity, Some(8));
}

#[test]
fn enqueue_hints_prefers_structured_campaign_and_tier_over_description_tags() {
    let mut params = base_params("campaign:desc tier:issue_repair");
    params.campaign_id = Some("structured".to_string());
    params.benchmark_tier = Some("crate_regen".to_string());
    let hints = submission::enqueue_hints_from_submit_params(&params).expect("hints");
    assert_eq!(hints.campaign_id.as_deref(), Some("structured"));
    assert_eq!(
        hints.benchmark_tier,
        Some(vox_orchestrator::ReconstructionBenchmarkTier::CrateRegen)
    );
}

#[test]
fn socrates_context_from_retrieval_preserves_verification_and_quality_signals() {
    let retrieval = crate::memory::RetrievalEvidenceEnvelope {
        trigger: crate::memory::RetrievalTriggerMode::ExplicitToolQuery,
        retrieval_tier: "hybrid".to_string(),
        memory_hit_count: 2,
        knowledge_hit_count: 1,
        chunk_hit_count: 1,
        repo_hit_count: 1,
        used_vector: true,
        used_bm25: true,
        used_lexical_fallback: false,
        contradiction_count: 1,
        top_score: Some(0.73),
        search_intent: "factual_lookup".to_string(),
        selected_mode: "hybrid".to_string(),
        backend_mix: vec!["memory_vector".to_string(), "chunk_fts".to_string()],
        source_diversity: 3,
        evidence_quality: 0.68,
        citation_coverage: 0.75,
        verification_performed: true,
        verification_reason: Some("weak_evidence_quality".to_string()),
        verification_query: Some("alpha beta".to_string()),
        recommended_next_action: Some("focus_codex".to_string()),
        search_plan: serde_json::json!({ "intent": "factual_lookup" }),
        search_diagnostics: serde_json::json!({ "verification_performed": true }),
        sqlite_journal_mode: None,
        sqlite_fts5_reported: None,
        sqlite_foreign_keys_on: None,
        rrf_fused_hit_count: 0,
    };
    let ctx = submission::socrates_context_from_retrieval(&retrieval);
    assert_eq!(ctx.source_diversity, 3);
    assert!((ctx.evidence_quality - 0.68).abs() < f64::EPSILON);
    assert!((ctx.citation_coverage - 0.75).abs() < f64::EPSILON);
    assert!(ctx.verification_performed);
    assert_eq!(
        ctx.verification_reason.as_deref(),
        Some("weak_evidence_quality")
    );
    assert_eq!(ctx.recommended_next_action.as_deref(), Some("focus_codex"));
}
