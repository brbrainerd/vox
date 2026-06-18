//! Wave 2 Track F — discovery bridge integration tests.

use vox_research_shim::research::discovery_bridge::{
    PersistFindingOutcome, build_manifest_metadata_json,
    parse_research_session_ids_from_metadata_json, persist_finding_candidate_from_research,
};
use vox_research_shim::research::types::{
    ResearchMetadata, ResearchResult, RetrievalDiagnostics, RoutingTier,
};
use vox_research_shim::research::verifier::{ClaimVerdict, EvidenceSpan, SpanType, Verdict};
use vox_research_shim::research::{claims::Claim, types::ResearchScope};

fn supported_result(session_id: i64, quality: i32) -> ResearchResult {
    ResearchResult {
        answer: "Discovery bridge integration answer.".to_string(),
        sources: vec![],
        citations: vec![],
        research_metadata: ResearchMetadata {
            session_id,
            duration_ms: 1,
            provider: "test".to_string(),
            routing_tier: RoutingTier::Direct,
            confidence: 0.8,
            subquery_count: 1,
            source_count: 0,
            claim_verdicts: vec![ClaimVerdict {
                claim: Claim {
                    claim_id: 99,
                    text: "supported".to_string(),
                    is_numeric: false,
                    is_recent: false,
                    is_named_event: false,
                },
                verdict: Verdict::Supported,
                confidence: 0.95,
                supporting_count: 1,
                contradicting_count: 0,
                evidence_spans: vec![EvidenceSpan {
                    source_id: 0,
                    span_start: 0,
                    span_end: 9,
                    text: "supported".to_string(),
                    span_type: SpanType::Supporting,
                }],
            }],
            retrieval_diagnostics: RetrievalDiagnostics::default(),
            quality_score: quality,
            planner_degraded: false,
            competence: None,
            self_verification: None,
            citation_audit: None,
        },
    }
}

#[test]
fn manifest_metadata_json_links_research_session_ids() {
    let json = build_manifest_metadata_json(&[42]);
    assert_eq!(
        parse_research_session_ids_from_metadata_json(&json),
        vec![42]
    );
}

#[test]
fn research_metadata_planner_degraded_serializes_in_json() {
    let mut result = supported_result(1, 60);
    result.research_metadata.planner_degraded = true;
    let json = serde_json::to_value(&result.research_metadata).expect("serializes");
    assert_eq!(json["planner_degraded"], true);
}

#[tokio::test]
async fn persist_finding_candidate_idempotent_round_trip() {
    let db = vox_db::VoxDb::connect(vox_db::DbConfig::Memory)
        .await
        .expect("memory db");
    let sid = 77_i64;

    let result = supported_result(sid, 60);
    assert_eq!(
        persist_finding_candidate_from_research(&result, &db)
            .await
            .expect("insert"),
        PersistFindingOutcome::Inserted
    );
    assert_eq!(
        persist_finding_candidate_from_research(&result, &db)
            .await
            .expect("already seen"),
        PersistFindingOutcome::AlreadySeen
    );

    let manifest_json = build_manifest_metadata_json(&[sid]);
    assert_eq!(
        parse_research_session_ids_from_metadata_json(&manifest_json),
        vec![sid]
    );

    let candidate_id =
        vox_research_shim::research::discovery_bridge::finding_candidate_from_research_result(
            &result,
        )
        .candidate_id;
    let row = db
        .get_finding_candidate(&candidate_id)
        .await
        .expect("get row")
        .expect("row present");
    let confidence: serde_json::Value =
        serde_json::from_str(row.confidence_json.as_deref().unwrap_or("{}"))
            .expect("confidence json");
    assert_eq!(confidence["research_session_ids"], serde_json::json!([sid]));
    let embedded = confidence["manifest_metadata_json"]
        .as_str()
        .expect("manifest_metadata_json embedded");
    assert_eq!(
        parse_research_session_ids_from_metadata_json(embedded),
        vec![sid]
    );
}

#[test]
fn research_scope_import_smoke() {
    let _ = ResearchScope::Both;
}
