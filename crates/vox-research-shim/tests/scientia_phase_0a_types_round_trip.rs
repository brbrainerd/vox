//! Phase 0a — types must round-trip through serde for telemetry persistence.

use vox_research_shim::research::types::*;

#[test]
fn research_query_default_constructs() {
    let q = ResearchQuery {
        query: "test".to_string(),
        scope: ResearchScope::Both,
        max_sources: 5,
        persist_to_docs: false,
        verify_claims: true,
        site_scope: None,
    };
    assert_eq!(q.query, "test");
    assert_eq!(q.max_sources, 5);
}

#[test]
fn retrieval_diagnostics_serializes() {
    let d = RetrievalDiagnostics {
        coverage_pct: 0.5,
        subquery_coverage_pct: 0.5,
        avg_provider_score: 0.0,
        fusion_weights: (0.0, 0.0, 0.0),
        dropped_source_count: 0,
        hit_rate: 0.0,
        distinct_domain_count: 2,
        citation_diversity_below_threshold: true,
    };
    let json = serde_json::to_value(&d).expect("serializes");
    assert!(json.is_object());
    let weights = &json["fusion_weights"];
    assert!(
        weights.is_array() && weights.as_array().unwrap().len() == 3,
        "fusion_weights must serialize as a 3-element array"
    );
}

#[test]
fn research_metadata_planner_degraded_round_trips_json() {
    let meta = ResearchMetadata {
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
        planner_degraded: true,
        competence: None,
        self_verification: None,
        citation_audit: None,
    };
    let json = serde_json::to_value(&meta).expect("serializes");
    assert_eq!(json["planner_degraded"], true);
    let back: ResearchMetadata = serde_json::from_value(json).expect("deserializes");
    assert!(back.planner_degraded);
}

#[test]
fn routing_tier_debug_repr_stable() {
    // pipeline.rs uses format!("{:?}", routing_tier) for telemetry;
    // changing the Debug repr is a breaking change.
    assert_eq!(format!("{:?}", RoutingTier::DeepResearch), "DeepResearch");
    assert_eq!(format!("{:?}", RoutingTier::Light), "Light");
    assert_eq!(format!("{:?}", RoutingTier::Direct), "Direct");
}
