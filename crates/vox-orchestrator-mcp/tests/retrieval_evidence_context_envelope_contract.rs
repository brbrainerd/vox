use std::path::PathBuf;

use vox_orchestrator_mcp::memory_tools::RetrievalEvidenceEnvelope;

#[test]
fn retrieval_evidence_projection_validates_against_context_envelope_schema() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let schema_path = root.join("../../contracts/communication/context-envelope.schema.json");
    let schema_text = std::fs::read_to_string(&schema_path).expect("read schema");
    let schema: serde_json::Value = serde_json::from_str(&schema_text).expect("parse schema");
    let validator = jsonschema::validator_for(&schema).expect("compile schema validator");

    let evidence = RetrievalEvidenceEnvelope {
        retrieval_tier: "hybrid".to_string(),
        memory_hit_count: 2,
        knowledge_hit_count: 1,
        chunk_hit_count: 1,
        repo_hit_count: 0,
        used_vector: true,
        used_bm25: true,
        used_lexical_fallback: false,
        contradiction_count: 0,
        source_diversity: 2,
        evidence_quality: 0.87,
        citation_coverage: 0.92,
        verification_performed: true,
        verification_reason: Some("contradiction_detected".to_string()),
        recommended_next_action: Some("retry_hybrid".to_string()),
        rrf_fused_hit_count: 1,
        ..Default::default()
    };

    let envelope =
        evidence.to_context_envelope("repo-contract-test", Some("session-contract-test"));
    let instance = serde_json::to_value(&envelope).expect("serialize context envelope");
    validator
        .validate(&instance)
        .expect("validate against schema");
}
